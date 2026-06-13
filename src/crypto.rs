use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::RngCore;
use sha2::{Sha256, Digest};

use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct CryptoService {
    cipher: Aes256Gcm,
}

impl CryptoService {
    pub fn new(key_material: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(key_material.as_bytes());
        let key_hash = hasher.finalize();

        let key = Key::<Aes256Gcm>::from_slice(&key_hash);
        let cipher = Aes256Gcm::new(key);

        Self { cipher }
    }

    pub fn encrypt(&self, plaintext: &str) -> AppResult<String> {
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| AppError::Internal(format!("Encryption failed: {}", e)))?;

        let mut result = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(BASE64.encode(result))
    }

    pub fn decrypt(&self, ciphertext_b64: &str) -> AppResult<String> {
        let data = BASE64
            .decode(ciphertext_b64)
            .map_err(|e| AppError::Internal(format!("Base64 decode failed: {}", e)))?;

        if data.len() < 12 {
            return Err(AppError::Internal("Invalid ciphertext".to_string()));
        }

        let nonce = Nonce::from_slice(&data[..12]);
        let ciphertext = &data[12..];

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| AppError::Internal(format!("Decryption failed: {}", e)))?;

        String::from_utf8(plaintext).map_err(|e| AppError::Internal(format!("Invalid UTF-8: {}", e)))
    }

    pub fn encrypt_option(&self, plaintext: &Option<String>) -> AppResult<Option<String>> {
        match plaintext {
            Some(s) if !s.is_empty() => Ok(Some(self.encrypt(s)?)),
            _ => Ok(None),
        }
    }

    pub fn decrypt_option(&self, ciphertext: &Option<String>) -> AppResult<Option<String>> {
        match ciphertext {
            Some(s) if !s.is_empty() => Ok(Some(self.decrypt(s)?)),
            _ => Ok(None),
        }
    }
}
