use sqlx::SqlitePool;

use crate::error::AppResult;
use crate::models::{Device, UpgradeTask, DeviceUpgrade, AuditLog, AppMetrics};

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

        let total_devices = Device::count_by_type(&self.pool, &task.device_type).await?;
        let percentage = task.get_current_gray_percentage();
        let target_count = ((total_devices as f64) * (percentage as f64) / 100.0).ceil() as i64;

        let already_assigned = DeviceUpgrade::count_total(&self.pool, task.id).await?;
        let needed = target_count - already_assigned;

        if needed <= 0 {
            return Ok(());
        }

        let devices = Device::list_by_type(
            &self.pool,
            &task.device_type,
            needed,
            already_assigned,
        )
        .await?;

        for device in devices {
            if !self.meets_constraints(&device, task) {
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
