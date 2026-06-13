use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StatusReport {
    pub id: i64,
    pub device_id: String,
    pub task_id: Option<i64>,
    pub status: String,
    pub firmware_version: Option<String>,
    pub progress: Option<i64>,
    pub error_message: Option<String>,
    pub battery_level: Option<i64>,
    pub wifi_signal: Option<i64>,
    pub reported_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct StatusReportRequest {
    pub task_id: Option<i64>,
    pub status: String,
    pub firmware_version: Option<String>,
    pub progress: Option<i64>,
    pub error_message: Option<String>,
    pub battery_level: Option<i64>,
    pub wifi_signal: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CheckVersionRequest {
    pub device_type: String,
    pub firmware_version: Option<String>,
    pub hardware_version: Option<String>,
    pub battery_level: Option<i64>,
    pub wifi_ssid: Option<String>,
    pub wifi_signal: Option<i64>,
}

impl StatusReport {
    pub async fn create(
        pool: &SqlitePool,
        device_id: &str,
        req: &StatusReportRequest,
    ) -> AppResult<Self> {
        let now = Utc::now();
        sqlx::query(
            r#"INSERT INTO status_reports (
                  device_id, task_id, status, firmware_version, progress,
                  error_message, battery_level, wifi_signal, reported_at
               ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#
        )
        .bind(device_id)
        .bind(req.task_id)
        .bind(&req.status)
        .bind(&req.firmware_version)
        .bind(req.progress)
        .bind(&req.error_message)
        .bind(req.battery_level)
        .bind(req.wifi_signal)
        .bind(now)
        .execute(pool)
        .await?;

        let id: i64 = sqlx::query_scalar("SELECT last_insert_rowid()")
            .fetch_one(pool)
            .await?;

        let report = sqlx::query_as::<_, StatusReport>(
            r#"SELECT id, device_id, task_id, status, firmware_version, progress,
                      error_message, battery_level, wifi_signal, reported_at
               FROM status_reports WHERE id = ?"#
        )
        .bind(id)
        .fetch_one(pool)
        .await?;

        Ok(report)
    }

    pub async fn list_by_device(
        pool: &SqlitePool,
        device_id: &str,
        limit: i64,
    ) -> AppResult<Vec<Self>> {
        let reports = sqlx::query_as::<_, StatusReport>(
            r#"SELECT id, device_id, task_id, status, firmware_version, progress,
                      error_message, battery_level, wifi_signal, reported_at
               FROM status_reports WHERE device_id = ? ORDER BY reported_at DESC LIMIT ?"#
        )
        .bind(device_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(reports)
    }

    pub async fn list_by_task(
        pool: &SqlitePool,
        task_id: i64,
        limit: i64,
    ) -> AppResult<Vec<Self>> {
        let reports = sqlx::query_as::<_, StatusReport>(
            r#"SELECT id, device_id, task_id, status, firmware_version, progress,
                      error_message, battery_level, wifi_signal, reported_at
               FROM status_reports WHERE task_id = ? ORDER BY reported_at DESC LIMIT ?"#
        )
        .bind(task_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(reports)
    }
}
