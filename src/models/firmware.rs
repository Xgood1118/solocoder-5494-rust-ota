use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Firmware {
    pub id: i64,
    pub version: String,
    pub device_type: String,
    pub file_path: String,
    pub file_size: i64,
    pub sha256_hash: String,
    pub rsa_signature: String,
    pub metadata: Option<String>,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFirmwareRequest {
    pub version: String,
    pub device_type: String,
    pub description: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct FirmwareResponse {
    pub id: i64,
    pub version: String,
    pub device_type: String,
    pub file_size: i64,
    pub sha256_hash: String,
    pub rsa_signature: String,
    pub metadata: Option<serde_json::Value>,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl Firmware {
    pub async fn find_by_id(pool: &SqlitePool, id: i64) -> AppResult<Option<Self>> {
        let fw = sqlx::query_as::<_, Firmware>(
            r#"SELECT id, version, device_type, file_path, file_size, sha256_hash,
                      rsa_signature, metadata, description, is_active, created_at
               FROM firmware WHERE id = ?"#
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(fw)
    }

    pub async fn find_by_version_and_type(
        pool: &SqlitePool,
        version: &str,
        device_type: &str,
    ) -> AppResult<Option<Self>> {
        let fw = sqlx::query_as::<_, Firmware>(
            r#"SELECT id, version, device_type, file_path, file_size, sha256_hash,
                      rsa_signature, metadata, description, is_active, created_at
               FROM firmware WHERE version = ? AND device_type = ?"#
        )
        .bind(version)
        .bind(device_type)
        .fetch_optional(pool)
        .await?;
        Ok(fw)
    }

    pub async fn find_latest_active(
        pool: &SqlitePool,
        device_type: &str,
    ) -> AppResult<Option<Self>> {
        let fw = sqlx::query_as::<_, Firmware>(
            r#"SELECT id, version, device_type, file_path, file_size, sha256_hash,
                      rsa_signature, metadata, description, is_active, created_at
               FROM firmware WHERE device_type = ? AND is_active = 1
               ORDER BY id DESC LIMIT 1"#
        )
        .bind(device_type)
        .fetch_optional(pool)
        .await?;
        Ok(fw)
    }

    pub async fn list_by_device_type(pool: &SqlitePool, device_type: &str) -> AppResult<Vec<Self>> {
        let list = sqlx::query_as::<_, Firmware>(
            r#"SELECT id, version, device_type, file_path, file_size, sha256_hash,
                      rsa_signature, metadata, description, is_active, created_at
               FROM firmware WHERE device_type = ? ORDER BY id DESC"#
        )
        .bind(device_type)
        .fetch_all(pool)
        .await?;
        Ok(list)
    }

    pub async fn create(
        pool: &SqlitePool,
        version: &str,
        device_type: &str,
        file_path: &str,
        file_size: i64,
        sha256_hash: &str,
        rsa_signature: &str,
        metadata: Option<&str>,
        description: Option<&str>,
    ) -> AppResult<Self> {
        sqlx::query(
            r#"INSERT INTO firmware (version, device_type, file_path, file_size,
                                     sha256_hash, rsa_signature, metadata, description, is_active)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0)"#
        )
        .bind(version)
        .bind(device_type)
        .bind(file_path)
        .bind(file_size)
        .bind(sha256_hash)
        .bind(rsa_signature)
        .bind(metadata)
        .bind(description)
        .execute(pool)
        .await?;

        Self::find_by_version_and_type(pool, version, device_type)
            .await?
            .ok_or_else(|| crate::error::AppError::Internal("Failed to create firmware".to_string()))
    }

    pub async fn set_active(pool: &SqlitePool, id: i64, is_active: bool) -> AppResult<()> {
        let active_int = if is_active { 1 } else { 0 };
        sqlx::query("UPDATE firmware SET is_active = ? WHERE id = ?")
            .bind(active_int)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
        sqlx::query("DELETE FROM firmware WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

impl From<Firmware> for FirmwareResponse {
    fn from(fw: Firmware) -> Self {
        let metadata = fw.metadata.as_deref()
            .and_then(|m| serde_json::from_str(m).ok());
        Self {
            id: fw.id,
            version: fw.version,
            device_type: fw.device_type,
            file_size: fw.file_size,
            sha256_hash: fw.sha256_hash,
            rsa_signature: fw.rsa_signature,
            metadata,
            description: fw.description,
            is_active: fw.is_active,
            created_at: fw.created_at,
        }
    }
}
