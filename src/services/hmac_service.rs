use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{AppError, AppResult};
use crate::models::{HmacKey, AuditLog, AppMetrics};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct HmacService {
    pub pool: SqlitePool,
    pub primary_key: String,
    pub secondary_key: String,
    pub url_expiry_secs: u64,
    pub metrics: AppMetrics,
}

impl HmacService {
    pub fn new(
        pool: SqlitePool,
        primary_key: String,
        secondary_key: String,
        url_expiry_secs: u64,
        metrics: AppMetrics,
    ) -> Self {
        Self {
            pool,
            primary_key,
            secondary_key,
            url_expiry_secs,
            metrics,
        }
    }

    pub fn generate_download_url(
        &self,
        firmware_id: i64,
        device_id: &str,
        base_url: &str,
    ) -> (String, String) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let expires = now + self.url_expiry_secs;
        let key_id = "primary";

        let message = format!("{}|{}|{}|{}", firmware_id, device_id, expires, key_id);

        let mut mac = HmacSha256::new_from_slice(self.primary_key.as_bytes())
            .expect("HMAC key length is valid");
        mac.update(message.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let url = format!(
            "{}/api/v1/firmware/{}/download?device_id={}&expires={}&key_id={}&sig={}",
            base_url, firmware_id, device_id, expires, key_id, signature
        );

        (url, key_id.to_string())
    }

    pub fn verify_download_signature(
        &self,
        firmware_id: i64,
        device_id: &str,
        expires: u64,
        key_id: &str,
        signature: &str,
    ) -> AppResult<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if expires < now {
            return Err(AppError::UrlExpired);
        }

        let message = format!("{}|{}|{}|{}", firmware_id, device_id, expires, key_id);

        let key = match key_id {
            "primary" => &self.primary_key,
            "secondary" => &self.secondary_key,
            _ => return Err(AppError::InvalidSignature("Unknown key_id".to_string())),
        };

        let mut mac = HmacSha256::new_from_slice(key.as_bytes())
            .expect("HMAC key length is valid");
        mac.update(message.as_bytes());
        let expected = hex::encode(mac.finalize().into_bytes());

        if !constant_time_eq(signature.as_bytes(), expected.as_bytes()) {
            return Err(AppError::InvalidSignature("Signature mismatch".to_string()));
        }

        Ok(())
    }

    pub async fn rotate_keys(&self, new_primary: String) -> AppResult<()> {
        let old_primary = HmacKey::find_primary(&self.pool).await?;

        if let Some(old) = &old_primary {
            let new_key_id = format!("key-{}", uuid::Uuid::new_v4());
            HmacKey::rotate_primary(
                &self.pool,
                &old.key_id,
                &new_key_id,
                &new_primary,
            )
            .await?;

            self.metrics.hmac_rotations.inc();

            AuditLog::create(
                &self.pool,
                "hmac_key_rotate",
                "system",
                "hmac_key",
                Some(&old.key_id),
                Some(&old.key_id),
                Some(&new_key_id),
                None,
                None,
            )
            .await?;
        }

        Ok(())
    }

    pub fn reload_keys(&mut self, pool_keys: (String, String)) {
        self.primary_key = pool_keys.0;
        self.secondary_key = pool_keys.1;
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}
