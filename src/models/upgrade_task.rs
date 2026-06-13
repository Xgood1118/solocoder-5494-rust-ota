use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UpgradeTask {
    pub id: i64,
    pub task_name: String,
    pub firmware_id: i64,
    pub device_type: String,
    pub gray_stage: i64,
    pub gray_percentages: String,
    pub min_battery_level: Option<i64>,
    pub require_wifi: bool,
    pub time_window_start: Option<String>,
    pub time_window_end: Option<String>,
    pub failure_rate_threshold: f64,
    pub status: String,
    pub auto_paused: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUpgradeTaskRequest {
    pub task_name: String,
    pub firmware_id: i64,
    pub gray_percentages: Option<Vec<i64>>,
    pub min_battery_level: Option<i64>,
    pub require_wifi: Option<bool>,
    pub time_window_start: Option<String>,
    pub time_window_end: Option<String>,
    pub failure_rate_threshold: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskStatusRequest {
    pub status: String,
}

impl UpgradeTask {
    pub async fn find_by_id(pool: &SqlitePool, id: i64) -> AppResult<Option<Self>> {
        let task = sqlx::query_as::<_, UpgradeTask>(
            r#"SELECT id, task_name, firmware_id, device_type, gray_stage, gray_percentages,
                      min_battery_level, require_wifi, time_window_start, time_window_end,
                      failure_rate_threshold, status, auto_paused, created_at, updated_at
               FROM upgrade_tasks WHERE id = ?"#
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(task)
    }

    pub async fn list_active(pool: &SqlitePool, device_type: &str) -> AppResult<Vec<Self>> {
        let tasks = sqlx::query_as::<_, UpgradeTask>(
            r#"SELECT id, task_name, firmware_id, device_type, gray_stage, gray_percentages,
                      min_battery_level, require_wifi, time_window_start, time_window_end,
                      failure_rate_threshold, status, auto_paused, created_at, updated_at
               FROM upgrade_tasks
               WHERE device_type = ? AND status IN ('running', 'paused')
               ORDER BY id DESC"#
        )
        .bind(device_type)
        .fetch_all(pool)
        .await?;
        Ok(tasks)
    }

    pub async fn list_all_active(pool: &SqlitePool) -> AppResult<Vec<Self>> {
        let tasks = sqlx::query_as::<_, UpgradeTask>(
            r#"SELECT id, task_name, firmware_id, device_type, gray_stage, gray_percentages,
                      min_battery_level, require_wifi, time_window_start, time_window_end,
                      failure_rate_threshold, status, auto_paused, created_at, updated_at
               FROM upgrade_tasks
               WHERE status IN ('running', 'paused')
               ORDER BY id DESC"#
        )
        .fetch_all(pool)
        .await?;
        Ok(tasks)
    }

    pub async fn list_all(pool: &SqlitePool) -> AppResult<Vec<Self>> {
        let tasks = sqlx::query_as::<_, UpgradeTask>(
            r#"SELECT id, task_name, firmware_id, device_type, gray_stage, gray_percentages,
                      min_battery_level, require_wifi, time_window_start, time_window_end,
                      failure_rate_threshold, status, auto_paused, created_at, updated_at
               FROM upgrade_tasks ORDER BY id DESC"#
        )
        .fetch_all(pool)
        .await?;
        Ok(tasks)
    }

    pub async fn create(pool: &SqlitePool, req: &CreateUpgradeTaskRequest, device_type: &str) -> AppResult<Self> {
        let now = Utc::now();
        let gray_percentages = req.gray_percentages
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[5,20,50,100]".to_string()))
            .unwrap_or_else(|| "[5,20,50,100]".to_string());
        let require_wifi = req.require_wifi.unwrap_or(false) as i64;
        let failure_rate_threshold = req.failure_rate_threshold.unwrap_or(0.1);

        sqlx::query(
            r#"INSERT INTO upgrade_tasks (
                  task_name, firmware_id, device_type, gray_stage, gray_percentages,
                  min_battery_level, require_wifi, time_window_start, time_window_end,
                  failure_rate_threshold, status, auto_paused, created_at, updated_at
               ) VALUES (?, ?, ?, 0, ?, ?, ?, ?, ?, ?, 'pending', 0, ?, ?)"#
        )
        .bind(&req.task_name)
        .bind(req.firmware_id)
        .bind(device_type)
        .bind(&gray_percentages)
        .bind(req.min_battery_level)
        .bind(require_wifi)
        .bind(&req.time_window_start)
        .bind(&req.time_window_end)
        .bind(failure_rate_threshold)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        let id: i64 = sqlx::query_scalar("SELECT last_insert_rowid()")
            .fetch_one(pool)
            .await?;

        Self::find_by_id(pool, id)
            .await?
            .ok_or_else(|| crate::error::AppError::Internal("Failed to create task".to_string()))
    }

    pub async fn update_status(pool: &SqlitePool, id: i64, status: &str) -> AppResult<()> {
        let now = Utc::now();
        sqlx::query("UPDATE upgrade_tasks SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn set_auto_paused(pool: &SqlitePool, id: i64, paused: bool) -> AppResult<()> {
        let now = Utc::now();
        let paused_int = if paused { 1 } else { 0 };
        sqlx::query("UPDATE upgrade_tasks SET auto_paused = ?, status = 'paused', updated_at = ? WHERE id = ?")
            .bind(paused_int)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn advance_gray_stage(pool: &SqlitePool, id: i64, new_stage: i64) -> AppResult<()> {
        let now = Utc::now();
        sqlx::query("UPDATE upgrade_tasks SET gray_stage = ?, updated_at = ? WHERE id = ?")
            .bind(new_stage)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub fn get_gray_percentages(&self) -> Vec<i64> {
        serde_json::from_str(&self.gray_percentages)
            .unwrap_or_else(|_| vec![5, 20, 50, 100])
    }

    pub fn get_current_gray_percentage(&self) -> i64 {
        let pcts = self.get_gray_percentages();
        let stage = self.gray_stage as usize;
        if stage < pcts.len() { pcts[stage] } else { 100 }
    }

    pub fn is_time_window_valid(&self) -> bool {
        use chrono::Timelike;
        let now = Utc::now();

        match (&self.time_window_start, &self.time_window_end) {
            (Some(start), Some(end)) => {
                let parse_hm = |s: &str| -> Option<(u32, u32)> {
                    let parts: Vec<&str> = s.split(':').collect();
                    if parts.len() == 2 {
                        let h = parts[0].parse::<u32>().ok()?;
                        let m = parts[1].parse::<u32>().ok()?;
                        Some((h, m))
                    } else {
                        None
                    }
                };

                match (parse_hm(start), parse_hm(end)) {
                    (Some((sh, sm)), Some((eh, em))) => {
                        let now_minutes = now.hour() * 60 + now.minute();
                        let start_minutes = sh * 60 + sm;
                        let end_minutes = eh * 60 + em;

                        if start_minutes <= end_minutes {
                            now_minutes >= start_minutes && now_minutes <= end_minutes
                        } else {
                            now_minutes >= start_minutes || now_minutes <= end_minutes
                        }
                    }
                    _ => true,
                }
            }
            _ => true,
        }
    }
}
