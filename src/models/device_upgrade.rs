use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DeviceUpgrade {
    pub id: i64,
    pub task_id: i64,
    pub device_id: String,
    pub firmware_id: i64,
    pub status: String,
    pub gray_stage: i64,
    pub failure_reason: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpgradeStatusUpdate {
    pub status: String,
    pub failure_reason: Option<String>,
}

fn is_valid_state_transition(from: &str, to: &str) -> bool {
    if from == to {
        return true;
    }

    match from {
        "pending" => matches!(to, "downloading" | "failed"),
        "downloading" => matches!(to, "installing" | "failed"),
        "installing" => matches!(to, "success" | "failed"),
        "success" => matches!(to, "rebooted"),
        "failed" => matches!(to, "downloading"),
        "rebooted" => false,
        _ => false,
    }
}

impl DeviceUpgrade {
    pub async fn find_by_task_and_device(
        pool: &SqlitePool,
        task_id: i64,
        device_id: &str,
    ) -> AppResult<Option<Self>> {
        let du = sqlx::query_as::<_, DeviceUpgrade>(
            r#"SELECT id, task_id, device_id, firmware_id, status, gray_stage,
                      failure_reason, started_at, completed_at, created_at, updated_at
               FROM device_upgrades WHERE task_id = ? AND device_id = ?"#
        )
        .bind(task_id)
        .bind(device_id)
        .fetch_optional(pool)
        .await?;
        Ok(du)
    }

    pub async fn find_pending_for_device(
        pool: &SqlitePool,
        device_id: &str,
    ) -> AppResult<Vec<Self>> {
        let list = sqlx::query_as::<_, DeviceUpgrade>(
            r#"SELECT id, task_id, device_id, firmware_id, status, gray_stage,
                      failure_reason, started_at, completed_at, created_at, updated_at
               FROM device_upgrades
               WHERE device_id = ? AND status IN ('pending', 'downloading', 'installing', 'failed')
               ORDER BY id DESC"#
        )
        .bind(device_id)
        .fetch_all(pool)
        .await?;
        Ok(list)
    }

    pub async fn create(
        pool: &SqlitePool,
        task_id: i64,
        device_id: &str,
        firmware_id: i64,
        gray_stage: i64,
    ) -> AppResult<Self> {
        let now = Utc::now();

        let existing = Self::find_by_task_and_device(pool, task_id, device_id).await?;
        if existing.is_some() {
            return Ok(existing.unwrap());
        }

        sqlx::query(
            r#"INSERT INTO device_upgrades (task_id, device_id, firmware_id, status, gray_stage, created_at, updated_at)
               VALUES (?, ?, ?, 'pending', ?, ?, ?)"#
        )
        .bind(task_id)
        .bind(device_id)
        .bind(firmware_id)
        .bind(gray_stage)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        Self::find_by_task_and_device(pool, task_id, device_id)
            .await?
            .ok_or_else(|| crate::error::AppError::Internal("Failed to create device upgrade".to_string()))
    }

    pub async fn update_status(
        pool: &SqlitePool,
        id: i64,
        status: &str,
        failure_reason: Option<&str>,
    ) -> AppResult<()> {
        let now = Utc::now();
        let (started_at, completed_at) = match status {
            "downloading" | "installing" => (Some(now), None),
            "success" | "failed" | "rebooted" => (None, Some(now)),
            _ => (None, None),
        };

        sqlx::query(
            r#"UPDATE device_upgrades
               SET status = ?,
                   failure_reason = COALESCE(?, failure_reason),
                   started_at = COALESCE(?, started_at),
                   completed_at = COALESCE(?, completed_at),
                   updated_at = ?
               WHERE id = ?"#
        )
        .bind(status)
        .bind(failure_reason)
        .bind(started_at)
        .bind(completed_at)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn transition_status(
        pool: &SqlitePool,
        id: i64,
        new_status: &str,
        failure_reason: Option<&str>,
    ) -> AppResult<String> {
        let current = sqlx::query_as::<_, DeviceUpgrade>(
            r#"SELECT id, task_id, device_id, firmware_id, status, gray_stage,
                      failure_reason, started_at, completed_at, created_at, updated_at
               FROM device_upgrades WHERE id = ?"#
        )
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(format!("Device upgrade {} not found", id)))?;

        if !is_valid_state_transition(&current.status, new_status) {
            return Err(crate::error::AppError::BadRequest(format!(
                "Invalid state transition: {} -> {}",
                current.status, new_status
            )));
        }

        Self::update_status(pool, id, new_status, failure_reason).await?;

        Ok(current.status)
    }

    pub async fn count_by_status(pool: &SqlitePool, task_id: i64, status: &str) -> AppResult<i64> {
        let count: Option<i64> = sqlx::query_scalar(
            "SELECT COUNT(*) FROM device_upgrades WHERE task_id = ? AND status = ?"
        )
        .bind(task_id)
        .bind(status)
        .fetch_one(pool)
        .await?;
        Ok(count.unwrap_or(0))
    }

    pub async fn count_total(pool: &SqlitePool, task_id: i64) -> AppResult<i64> {
        let count: Option<i64> = sqlx::query_scalar(
            "SELECT COUNT(*) FROM device_upgrades WHERE task_id = ?"
        )
        .bind(task_id)
        .fetch_one(pool)
        .await?;
        Ok(count.unwrap_or(0))
    }

    pub async fn list_by_task(pool: &SqlitePool, task_id: i64, limit: i64, offset: i64) -> AppResult<Vec<Self>> {
        let list = sqlx::query_as::<_, DeviceUpgrade>(
            r#"SELECT id, task_id, device_id, firmware_id, status, gray_stage,
                      failure_reason, started_at, completed_at, created_at, updated_at
               FROM device_upgrades WHERE task_id = ? ORDER BY id DESC LIMIT ? OFFSET ?"#
        )
        .bind(task_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(list)
    }

    pub async fn calculate_failure_rate(pool: &SqlitePool, task_id: i64) -> AppResult<f64> {
        let total: Option<i64> = sqlx::query_scalar(
            "SELECT COUNT(*) FROM device_upgrades WHERE task_id = ? AND status IN ('success', 'failed')"
        )
        .bind(task_id)
        .fetch_one(pool)
        .await?;

        let failed: Option<i64> = sqlx::query_scalar(
            "SELECT COUNT(*) FROM device_upgrades WHERE task_id = ? AND status = 'failed'"
        )
        .bind(task_id)
        .fetch_one(pool)
        .await?;

        let total = total.unwrap_or(0);
        let failed = failed.unwrap_or(0);

        if total == 0 {
            Ok(0.0)
        } else {
            Ok(failed as f64 / total as f64)
        }
    }
}
