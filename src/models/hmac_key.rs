use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct HmacKey {
    pub id: i64,
    pub key_id: String,
    pub secret_key: String,
    pub is_active: bool,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct RotateKeyRequest {
    pub new_secret_key: String,
}

impl HmacKey {
    pub async fn find_all_active(pool: &SqlitePool) -> AppResult<Vec<Self>> {
        let now = Utc::now();
        let keys = sqlx::query_as::<_, HmacKey>(
            r#"SELECT id, key_id, secret_key, is_active, is_primary, created_at, expires_at
               FROM hmac_keys
               WHERE is_active = 1 AND (expires_at IS NULL OR expires_at > ?)
               ORDER BY is_primary DESC"#
        )
        .bind(now)
        .fetch_all(pool)
        .await?;
        Ok(keys)
    }

    pub async fn find_primary(pool: &SqlitePool) -> AppResult<Option<Self>> {
        let key = sqlx::query_as::<_, HmacKey>(
            r#"SELECT id, key_id, secret_key, is_active, is_primary, created_at, expires_at
               FROM hmac_keys WHERE is_primary = 1 AND is_active = 1 LIMIT 1"#
        )
        .fetch_optional(pool)
        .await?;
        Ok(key)
    }

    pub async fn find_by_key_id(pool: &SqlitePool, key_id: &str) -> AppResult<Option<Self>> {
        let key = sqlx::query_as::<_, HmacKey>(
            r#"SELECT id, key_id, secret_key, is_active, is_primary, created_at, expires_at
               FROM hmac_keys WHERE key_id = ?"#
        )
        .bind(key_id)
        .fetch_optional(pool)
        .await?;
        Ok(key)
    }

    pub async fn rotate_primary(
        pool: &SqlitePool,
        old_key_id: &str,
        new_key_id: &str,
        new_secret_key: &str,
    ) -> AppResult<()> {
        let now = Utc::now();
        let expire_time = now + chrono::Duration::hours(24);

        sqlx::query("UPDATE hmac_keys SET is_primary = 0, expires_at = ? WHERE key_id = ?")
            .bind(expire_time)
            .bind(old_key_id)
            .execute(pool)
            .await?;

        sqlx::query(
            r#"INSERT INTO hmac_keys (key_id, secret_key, is_active, is_primary, created_at)
               VALUES (?, ?, 1, 1, ?)"#
        )
        .bind(new_key_id)
        .bind(new_secret_key)
        .bind(now)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn deactivate(pool: &SqlitePool, key_id: &str) -> AppResult<()> {
        sqlx::query("UPDATE hmac_keys SET is_active = 0 WHERE key_id = ?")
            .bind(key_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
