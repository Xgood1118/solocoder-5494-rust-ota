use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    pub firmware_dir: String,
    pub rsa_private_key_pem: Option<String>,
    pub rsa_public_key_pem: Option<String>,
    pub download_url_expiry_secs: u64,
    pub hmac_primary_key: String,
    pub hmac_secondary_key: String,
    pub encryption_key: String,
}

impl Config {
    pub fn from_env() -> Self {
        let port = env::var("PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()
            .expect("PORT must be a valid number");

        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:ota.db?mode=rwc".to_string());

        let firmware_dir = env::var("FIRMWARE_DIR")
            .unwrap_or_else(|_| "./firmware_store".to_string());

        let rsa_private_key_pem = env::var("RSA_PRIVATE_KEY_PEM").ok();
        let rsa_public_key_pem = env::var("RSA_PUBLIC_KEY_PEM").ok();

        let download_url_expiry_secs = env::var("DOWNLOAD_URL_EXPIRY_SECS")
            .unwrap_or_else(|_| "3600".to_string())
            .parse()
            .expect("DOWNLOAD_URL_EXPIRY_SECS must be a valid number");

        let hmac_primary_key = env::var("HMAC_PRIMARY_KEY")
            .unwrap_or_else(|_| "primary-secret-key-change-in-production".to_string());

        let hmac_secondary_key = env::var("HMAC_SECONDARY_KEY")
            .unwrap_or_else(|_| "secondary-secret-key-change-in-production".to_string());

        let encryption_key = env::var("ENCRYPTION_KEY")
            .unwrap_or_else(|_| "default-encryption-key-change-in-production-please".to_string());

        Self {
            port,
            database_url,
            firmware_dir,
            rsa_private_key_pem,
            rsa_public_key_pem,
            download_url_expiry_secs,
            hmac_primary_key,
            hmac_secondary_key,
            encryption_key,
        }
    }
}
