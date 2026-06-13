use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Device {
    pub id: i64,
    pub device_id: String,
    pub device_type: String,
    pub firmware_version: Option<String>,
    pub hardware_version: Option<String>,
    pub battery_level: Option<i64>,
    pub wifi_ssid: Option<String>,
    pub wifi_signal: Option<i64>,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDeviceRequest {
    pub device_id: String,
    pub device_type: String,
    pub firmware_version: Option<String>,
    pub hardware_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDeviceRequest {
    pub firmware_version: Option<String>,
    pub battery_level: Option<i64>,
    pub wifi_ssid: Option<String>,
    pub wifi_signal: Option<i64>,
}

impl Device {
    pub async fn find_by_device_id(pool: &SqlitePool, device_id: &str) -> AppResult<Option<Self>> {
        let device = sqlx::query_as::<_, Device>(
            r#"SELECT id, device_id, device_type, firmware_version, hardware_version,
                      battery_level, wifi_ssid, wifi_signal, last_heartbeat_at,
                      created_at, updated_at
               FROM devices WHERE device_id = ?"#
        )
        .bind(device_id)
        .fetch_optional(pool)
        .await?;

        Ok(device)
    }

    pub async fn create(pool: &SqlitePool, req: &CreateDeviceRequest) -> AppResult<Self> {
        let now = Utc::now();

        sqlx::query(
            r#"INSERT INTO devices (device_id, device_type, firmware_version, hardware_version,
                                    created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?)"#
        )
        .bind(&req.device_id)
        .bind(&req.device_type)
        .bind(&req.firmware_version)
        .bind(&req.hardware_version)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        let device = Self::find_by_device_id(pool, &req.device_id)
            .await?
            .unwrap();

        Ok(device)
    }

    pub async fn upsert(pool: &SqlitePool, device_id: &str, device_type: &str) -> AppResult<Self> {
        let existing = Self::find_by_device_id(pool, device_id).await?;

        if let Some(device) = existing {
            Ok(device)
        } else {
            let req = CreateDeviceRequest {
                device_id: device_id.to_string(),
                device_type: device_type.to_string(),
                firmware_version: None,
                hardware_version: None,
            };
            Self::create(pool, &req).await
        }
    }

    pub async fn update(pool: &SqlitePool, device_id: &str, req: &UpdateDeviceRequest) -> AppResult<Self> {
        let now = Utc::now();

        sqlx::query(
            r#"UPDATE devices
               SET firmware_version = COALESCE(?, firmware_version),
                   battery_level = COALESCE(?, battery_level),
                   wifi_ssid = COALESCE(?, wifi_ssid),
                   wifi_signal = COALESCE(?, wifi_signal),
                   last_heartbeat_at = ?,
                   updated_at = ?
               WHERE device_id = ?"#
        )
        .bind(&req.firmware_version)
        .bind(req.battery_level)
        .bind(&req.wifi_ssid)
        .bind(req.wifi_signal)
        .bind(now)
        .bind(now)
        .bind(device_id)
        .execute(pool)
        .await?;

        let device = Self::find_by_device_id(pool, device_id)
            .await?
            .unwrap();

        Ok(device)
    }

    pub async fn heartbeat(pool: &SqlitePool, device_id: &str) -> AppResult<()> {
        let now = Utc::now();
        sqlx::query("UPDATE devices SET last_heartbeat_at = ?, updated_at = ? WHERE device_id = ?")
            .bind(now)
            .bind(now)
            .bind(device_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn list_by_type(pool: &SqlitePool, device_type: &str, limit: i64, offset: i64) -> AppResult<Vec<Self>> {
        let devices = sqlx::query_as::<_, Device>(
            r#"SELECT id, device_id, device_type, firmware_version, hardware_version,
                      battery_level, wifi_ssid, wifi_signal, last_heartbeat_at,
                      created_at, updated_at
               FROM devices WHERE device_type = ? ORDER BY id LIMIT ? OFFSET ?"#
        )
        .bind(device_type)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(devices)
    }

    pub async fn count_by_type(pool: &SqlitePool, device_type: &str) -> AppResult<i64> {
        let count: Option<i64> = sqlx::query_scalar(
            "SELECT COUNT(*) FROM devices WHERE device_type = ?"
        )
        .bind(device_type)
        .fetch_one(pool)
        .await?;
        Ok(count.unwrap_or(0))
    }
}
