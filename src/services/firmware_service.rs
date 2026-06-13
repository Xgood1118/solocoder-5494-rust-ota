use std::path::{Path, PathBuf};
use tokio::fs;
use sha2::{Sha256, Digest};
use rsa::{RsaPrivateKey, RsaPublicKey, Pkcs1v15Sign};
use rsa::pkcs8::{DecodePublicKey, DecodePrivateKey};
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};
use crate::models::{Firmware, AuditLog, AppMetrics};

#[derive(Clone)]
pub struct FirmwareService {
    pub pool: SqlitePool,
    pub firmware_dir: PathBuf,
    pub rsa_private_key: Option<RsaPrivateKey>,
    pub rsa_public_key: Option<RsaPublicKey>,
    pub metrics: AppMetrics,
}

impl FirmwareService {
    pub fn new(
        pool: SqlitePool,
        firmware_dir: PathBuf,
        private_key_pem: Option<&str>,
        public_key_pem: Option<&str>,
        metrics: AppMetrics,
    ) -> AppResult<Self> {
        let rsa_private_key = private_key_pem
            .map(|pem| RsaPrivateKey::from_pkcs8_pem(pem))
            .transpose()
            .map_err(|e| AppError::Rsa(format!("Invalid private key: {}", e)))?;

        let rsa_public_key = public_key_pem
            .map(|pem| RsaPublicKey::from_public_key_pem(pem))
            .transpose()
            .map_err(|e| AppError::Rsa(format!("Invalid public key: {}", e)))?;

        Ok(Self {
            pool,
            firmware_dir,
            rsa_private_key,
            rsa_public_key,
            metrics,
        })
    }

    pub async fn ensure_dir(&self) -> AppResult<()> {
        fs::create_dir_all(&self.firmware_dir).await?;
        Ok(())
    }

    pub async fn upload(
        &self,
        version: &str,
        device_type: &str,
        data: &[u8],
        description: Option<&str>,
        metadata: Option<&str>,
    ) -> AppResult<Firmware> {
        self.ensure_dir().await?;

        let dir = self.firmware_dir.join(device_type);
        fs::create_dir_all(&dir).await?;

        let filename = format!("v{}_{}.bin", version, uuid::Uuid::new_v4());
        let file_path = dir.join(&filename);

        fs::write(&file_path, data).await?;

        let file_size = data.len() as i64;

        let mut hasher = Sha256::new();
        hasher.update(data);
        let sha256_hash = hex::encode(hasher.finalize());

        let rsa_signature = self.sign_firmware(data)?;

        let fw = Firmware::create(
            &self.pool,
            version,
            device_type,
            &file_path.to_string_lossy(),
            file_size,
            &sha256_hash,
            &rsa_signature,
            metadata,
            description,
        )
        .await?;

        self.metrics.firmware_uploads.inc();

        AuditLog::create(
            &self.pool,
            "firmware_upload",
            "system",
            "firmware",
            Some(&fw.id.to_string()),
            None,
            Some(&serde_json::json!({
                "version": version,
                "device_type": device_type,
                "sha256": sha256_hash,
            }).to_string()),
            None,
            None,
        )
        .await?;

        Ok(fw)
    }

    fn sign_firmware(&self, data: &[u8]) -> AppResult<String> {
        if let Some(ref private_key) = self.rsa_private_key {
            let mut hasher = Sha256::new();
            hasher.update(data);
            let hash = hasher.finalize();

            let signature = private_key
                .sign(Pkcs1v15Sign::new::<Sha256>(), &hash)
                .map_err(|e| AppError::Rsa(format!("RSA signing failed: {}", e)))?;

            Ok(hex::encode(signature))
        } else {
            Ok("no-rsa-key-configured".to_string())
        }
    }

    pub fn verify_signature(&self, data: &[u8], signature_hex: &str) -> AppResult<bool> {
        let public_key = self.rsa_public_key.as_ref()
            .ok_or_else(|| AppError::Rsa("No public key configured".to_string()))?;

        let signature = hex::decode(signature_hex)
            .map_err(|e| AppError::InvalidSignature(format!("Invalid hex: {}", e)))?;

        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hasher.finalize();

        match public_key.verify(Pkcs1v15Sign::new::<Sha256>(), &hash, &signature) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub async fn read_file(&self, file_path: &str) -> AppResult<Vec<u8>> {
        let path = Path::new(file_path);
        if !path.starts_with(&self.firmware_dir) {
            return Err(AppError::BadRequest("Invalid file path".to_string()));
        }
        let data = fs::read(path).await?;
        Ok(data)
    }

    pub async fn read_file_range(&self, file_path: &str, start: u64, end: u64) -> AppResult<Vec<u8>> {
        let path = Path::new(file_path);
        if !path.starts_with(&self.firmware_dir) {
            return Err(AppError::BadRequest("Invalid file path".to_string()));
        }

        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        let mut file = fs::File::open(path).await?;
        file.seek(std::io::SeekFrom::Start(start)).await?;

        let len = (end - start + 1) as usize;
        let mut buf = vec![0u8; len];
        file.read_exact(&mut buf).await?;

        Ok(buf)
    }

    pub async fn get_file_size(&self, file_path: &str) -> AppResult<u64> {
        let path = Path::new(file_path);
        if !path.starts_with(&self.firmware_dir) {
            return Err(AppError::BadRequest("Invalid file path".to_string()));
        }
        let meta = fs::metadata(path).await?;
        Ok(meta.len())
    }

    pub async fn delete_firmware(&self, id: i64) -> AppResult<()> {
        let fw = Firmware::find_by_id(&self.pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Firmware {} not found", id)))?;

        let path = Path::new(&fw.file_path);
        if path.exists() {
            fs::remove_file(path).await?;
        }

        Firmware::delete(&self.pool, id).await?;

        AuditLog::create(
            &self.pool,
            "firmware_delete",
            "system",
            "firmware",
            Some(&id.to_string()),
            Some(&serde_json::json!({"version": fw.version}).to_string()),
            None,
            None,
            None,
        )
        .await?;

        Ok(())
    }
}
