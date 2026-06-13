use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use std::sync::Arc;

use crate::AppState;
use crate::error::AppResult;
use crate::models::{Device, Firmware, StatusReport, StatusReportRequest, CheckVersionRequest, DeviceUpgrade, UpgradeTask};
use crate::models::device::UpdateDeviceRequest;

#[derive(Serialize)]
pub struct CheckVersionResponse {
    pub has_update: bool,
    pub firmware: Option<FirmwareInfo>,
    pub download_url: Option<String>,
    pub task_id: Option<i64>,
}

#[derive(Serialize)]
pub struct FirmwareInfo {
    pub id: i64,
    pub version: String,
    pub device_type: String,
    pub file_size: i64,
    pub sha256_hash: String,
    pub rsa_signature: String,
}

pub async fn check_version(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
    Json(req): Json<CheckVersionRequest>,
) -> AppResult<Json<CheckVersionResponse>> {
    let device = Device::upsert(&state.pool, &device_id, &req.device_type).await?;

    if device.status != "active" {
        return Ok(Json(CheckVersionResponse {
            has_update: false,
            firmware: None,
            download_url: None,
            task_id: None,
        }));
    }

    let pending = DeviceUpgrade::find_pending_for_device(&state.pool, &device_id).await?;

    let base_url = format!("http://localhost:{}", state.config.port);

    if let Some(upgrade) = pending.first() {
        let task = UpgradeTask::find_by_id(&state.pool, upgrade.task_id).await?;

        if let Some(task) = task {
            if task.status == "running" && !task.auto_paused {
                if !check_device_constraints(&device, &task, &req) {
                    let update_req = UpdateDeviceRequest {
                        firmware_version: req.firmware_version.clone(),
                        battery_level: req.battery_level,
                        wifi_ssid: req.wifi_ssid.clone(),
                        wifi_signal: req.wifi_signal,
                    };
                    Device::update(&state.pool, &device_id, &update_req).await?;

                    return Ok(Json(CheckVersionResponse {
                        has_update: false,
                        firmware: None,
                        download_url: None,
                        task_id: None,
                    }));
                }

                let fw = Firmware::find_by_id(&state.pool, upgrade.firmware_id).await?;

                if let Some(fw) = fw {
                    let update_req = UpdateDeviceRequest {
                        firmware_version: req.firmware_version.clone(),
                        battery_level: req.battery_level,
                        wifi_ssid: req.wifi_ssid.clone(),
                        wifi_signal: req.wifi_signal,
                    };
                    Device::update(&state.pool, &device_id, &update_req).await?;

                    let (download_url, _) = state.hmac_service.generate_download_url(
                        fw.id,
                        &device_id,
                        &base_url,
                    );

                    return Ok(Json(CheckVersionResponse {
                        has_update: true,
                        firmware: Some(FirmwareInfo {
                            id: fw.id,
                            version: fw.version,
                            device_type: fw.device_type,
                            file_size: fw.file_size,
                            sha256_hash: fw.sha256_hash,
                            rsa_signature: fw.rsa_signature,
                        }),
                        download_url: Some(download_url),
                        task_id: Some(upgrade.task_id),
                    }));
                }
            }
        }
    }

    let update_req = UpdateDeviceRequest {
        firmware_version: req.firmware_version.clone(),
        battery_level: req.battery_level,
        wifi_ssid: req.wifi_ssid.clone(),
        wifi_signal: req.wifi_signal,
    };
    Device::update(&state.pool, &device_id, &update_req).await?;

    let latest = Firmware::find_latest_active(&state.pool, &req.device_type).await?;

    match latest {
        Some(fw) => {
            let current_version = device.firmware_version.as_deref().unwrap_or("");
            let has_update = fw.version != current_version;

            let download_url = if has_update {
                let (url, _) = state.hmac_service.generate_download_url(
                    fw.id,
                    &device_id,
                    &base_url,
                );
                Some(url)
            } else {
                None
            };

            Ok(Json(CheckVersionResponse {
                has_update,
                firmware: Some(FirmwareInfo {
                    id: fw.id,
                    version: fw.version,
                    device_type: fw.device_type,
                    file_size: fw.file_size,
                    sha256_hash: fw.sha256_hash,
                    rsa_signature: fw.rsa_signature,
                }),
                download_url,
                task_id: None,
            }))
        }
        None => Ok(Json(CheckVersionResponse {
            has_update: false,
            firmware: None,
            download_url: None,
            task_id: None,
        })),
    }
}

fn check_device_constraints(
    device: &Device,
    task: &UpgradeTask,
    req: &CheckVersionRequest,
) -> bool {
    if !task.is_time_window_valid() {
        return false;
    }

    if let Some(min_battery) = task.min_battery_level {
        let battery = req.battery_level.or(device.battery_level).unwrap_or(0);
        if battery < min_battery {
            return false;
        }
    }

    if task.require_wifi {
        let wifi_ssid = req.wifi_ssid.as_deref().or(device.wifi_ssid.as_deref()).unwrap_or("");
        if wifi_ssid.is_empty() {
            return false;
        }
        let wifi_signal = req.wifi_signal.or(device.wifi_signal).unwrap_or(-100);
        if wifi_signal < -70 {
            return false;
        }
    }

    true
}

pub async fn report_status(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
    Json(req): Json<StatusReportRequest>,
) -> AppResult<Json<StatusReport>> {
    let report = StatusReport::create(&state.pool, &device_id, &req).await?;

    state.metrics.status_reports.inc();

    if let Some(task_id) = req.task_id {
        let upgrades = DeviceUpgrade::find_pending_for_device(&state.pool, &device_id)
            .await?;

        if let Some(upgrade) = upgrades.iter().find(|u| u.task_id == task_id) {
            let old_status = DeviceUpgrade::transition_status(
                &state.pool,
                upgrade.id,
                &req.status,
                req.error_message.as_deref(),
            )
            .await?;

            let new_status = req.status.as_str();
            match new_status {
                "downloading" => {
                    if old_status != "downloading" {
                        state.metrics.upgrade_downloading.inc();
                    }
                    if old_status == "installing" {
                        state.metrics.upgrade_installing.dec();
                    }
                }
                "installing" => {
                    if old_status != "installing" {
                        state.metrics.upgrade_installing.inc();
                    }
                    if old_status == "downloading" {
                        state.metrics.upgrade_downloading.dec();
                    }
                }
                "success" => {
                    state.metrics.upgrade_success.inc();
                    if old_status == "downloading" {
                        state.metrics.upgrade_downloading.dec();
                    }
                    if old_status == "installing" {
                        state.metrics.upgrade_installing.dec();
                    }
                }
                "failed" | "rebooted" => {
                    if new_status == "failed" {
                        state.metrics.upgrade_failed.inc();
                    }
                    if old_status == "downloading" {
                        state.metrics.upgrade_downloading.dec();
                    }
                    if old_status == "installing" {
                        state.metrics.upgrade_installing.dec();
                    }
                }
                _ => {}
            }
        }
    }

    Ok(Json(report))
}

pub async fn get_status_history(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
) -> AppResult<Json<Vec<StatusReport>>> {
    let reports = StatusReport::list_by_device(&state.pool, &device_id, 50).await?;
    Ok(Json(reports))
}
