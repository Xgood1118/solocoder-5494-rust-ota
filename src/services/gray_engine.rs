use sha2::{Sha256, Digest};
use sqlx::SqlitePool;

use crate::error::AppResult;
use crate::models::{Device, UpgradeTask, DeviceUpgrade, AuditLog, AppMetrics};

fn device_gray_percentage(device_id: &str) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(device_id.as_bytes());
    let hash = hasher.finalize();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash[..8]);
    let num = u64::from_be_bytes(bytes);
    num % 10000 / 100
}

fn is_device_in_gray(device_id: &str, percentage: i64) -> bool {
    let pct = percentage.max(0).min(100) as u64;
    if pct >= 100 {
        return true;
    }
    if pct == 0 {
        return false;
    }
    device_gray_percentage(device_id) < pct
}

#[derive(Clone)]
pub struct GrayEngine {
    pub pool: SqlitePool,
    pub metrics: AppMetrics,
}

impl GrayEngine {
    pub fn new(pool: SqlitePool, metrics: AppMetrics) -> Self {
        Self { pool, metrics }
    }

    pub async fn start_task(&self, task_id: i64) -> AppResult<()> {
        let task = UpgradeTask::find_by_id(&self.pool, task_id)
            .await?
            .ok_or_else(|| crate::error::AppError::NotFound(format!("Task {} not found", task_id)))?;

        UpgradeTask::update_status(&self.pool, task_id, "running").await?;

        self.assign_devices_for_stage(&task).await?;

        AuditLog::create(
            &self.pool,
            "task_start",
            "system",
            "upgrade_task",
            Some(&task_id.to_string()),
            None,
            Some(&serde_json::json!({"gray_stage": 0}).to_string()),
            None,
            None,
        )
        .await?;

        Ok(())
    }

    pub async fn advance_stage(&self, task_id: i64) -> AppResult<bool> {
        let task = UpgradeTask::find_by_id(&self.pool, task_id)
            .await?
            .ok_or_else(|| crate::error::AppError::NotFound(format!("Task {} not found", task_id)))?;

        let percentages = task.get_gray_percentages();
        let current_stage = task.gray_stage as usize;
        let next_stage = current_stage + 1;

        if next_stage >= percentages.len() {
            UpgradeTask::update_status(&self.pool, task_id, "completed").await?;
            return Ok(false);
        }

        UpgradeTask::advance_gray_stage(&self.pool, task_id, next_stage as i64).await?;

        let updated_task = UpgradeTask::find_by_id(&self.pool, task_id)
            .await?
            .unwrap();

        self.assign_devices_for_stage(&updated_task).await?;

        AuditLog::create(
            &self.pool,
            "task_advance_stage",
            "system",
            "upgrade_task",
            Some(&task_id.to_string()),
            Some(&serde_json::json!({"old_stage": current_stage}).to_string()),
            Some(&serde_json::json!({"new_stage": next_stage, "percentage": percentages[next_stage]}).to_string()),
            None,
            None,
        )
        .await?;

        Ok(true)
    }

    async fn assign_devices_for_stage(&self, task: &UpgradeTask) -> AppResult<()> {
        if !task.is_time_window_valid() {
            tracing::warn!(
                "Task {} outside time window, skipping device assignment",
                task.id
            );
            return Ok(());
        }

        let percentage = task.get_current_gray_percentage();
        let batch_size = 500;
        let mut offset = 0i64;
        let mut assigned_count = 0i64;

        loop {
            let devices = Device::list_by_type(
                &self.pool,
                &task.device_type,
                batch_size,
                offset,
            )
            .await?;

            if devices.is_empty() {
                break;
            }

            for device in &devices {
                if !is_device_in_gray(&device.device_id, percentage) {
                    continue;
                }

                if !self.meets_constraints(device, task) {
                    continue;
                }

                let existing = DeviceUpgrade::find_by_task_and_device(
                    &self.pool,
                    task.id,
                    &device.device_id,
                )
                .await?;

                if existing.is_some() {
                    continue;
                }

                DeviceUpgrade::create(
                    &self.pool,
                    task.id,
                    &device.device_id,
                    task.firmware_id,
                    task.gray_stage,
                )
                .await?;

                assigned_count += 1;
            }

            if devices.len() < batch_size as usize {
                break;
            }

            offset += batch_size;
        }

        if assigned_count > 0 {
            tracing::info!(
                "Task {} assigned {} devices for gray stage {} ({}%)",
                task.id,
                assigned_count,
                task.gray_stage,
                percentage
            );
        }

        Ok(())
    }

    fn meets_constraints(&self, device: &Device, task: &UpgradeTask) -> bool {
        if let Some(min_battery) = task.min_battery_level {
            if let Some(battery) = device.battery_level {
                if battery < min_battery {
                    return false;
                }
            } else {
                return false;
            }
        }

        if task.require_wifi {
            if device.wifi_ssid.is_none() || device.wifi_ssid.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                return false;
            }
            if let Some(signal) = device.wifi_signal {
                if signal < -70 {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }

    pub async fn pause_task(&self, task_id: i64, actor: &str) -> AppResult<()> {
        UpgradeTask::update_status(&self.pool, task_id, "paused").await?;

        AuditLog::create(
            &self.pool,
            "task_pause",
            actor,
            "upgrade_task",
            Some(&task_id.to_string()),
            Some("running"),
            Some("paused"),
            None,
            None,
        )
        .await?;

        Ok(())
    }

    pub async fn resume_task(&self, task_id: i64, actor: &str) -> AppResult<()> {
        UpgradeTask::update_status(&self.pool, task_id, "running").await?;

        AuditLog::create(
            &self.pool,
            "task_resume",
            actor,
            "upgrade_task",
            Some(&task_id.to_string()),
            Some("paused"),
            Some("running"),
            None,
            None,
        )
        .await?;

        Ok(())
    }
}
