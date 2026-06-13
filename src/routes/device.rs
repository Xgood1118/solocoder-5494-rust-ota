use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::AppState;
use crate::error::AppResult;
use crate::models::{Device, CreateDeviceRequest, UpdateDeviceRequest, AuditLog};

#[derive(Deserialize)]
pub struct DeviceStatusTransitionRequest {
    pub status: String,
}

pub async fn register_device(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateDeviceRequest>,
) -> AppResult<Json<Device>> {
    let device = Device::upsert(&state.pool, &req.device_id, &req.device_type).await?;

    if req.firmware_version.is_some() || req.hardware_version.is_some() {
        let update = UpdateDeviceRequest {
            firmware_version: req.firmware_version,
            battery_level: None,
            wifi_ssid: None,
            wifi_signal: None,
        };
        let device = Device::update(&state.pool, &req.device_id, &update).await?;
        Ok(Json(device))
    } else {
        Ok(Json(device))
    }
}

pub async fn update_device(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
    Json(req): Json<UpdateDeviceRequest>,
) -> AppResult<Json<Device>> {
    let device = Device::update(&state.pool, &device_id, &req).await?;
    Ok(Json(device))
}

pub async fn transition_device_status(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
    Json(req): Json<DeviceStatusTransitionRequest>,
) -> AppResult<Json<Device>> {
    let old_status = Device::transition_status(
        &state.pool,
        &device_id,
        &req.status,
        "api",
    )
    .await?;

    AuditLog::create(
        &state.pool,
        "device_status_transition",
        "api",
        "device",
        Some(&device_id),
        Some(&old_status),
        Some(&req.status),
        None,
        None,
    )
    .await?;

    let device = Device::find_by_device_id(&state.pool, &device_id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(format!("Device {} not found", device_id)))?;

    Ok(Json(device))
}

pub async fn heartbeat(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    Device::heartbeat(&state.pool, &device_id).await?;
    Ok(Json(serde_json::json!({"status": "ok"})))
}

pub async fn get_device(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
) -> AppResult<Json<Device>> {
    let device = Device::find_by_device_id(&state.pool, &device_id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(format!("Device {} not found", device_id)))?;
    Ok(Json(device))
}
