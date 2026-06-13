mod config;
mod crypto;
mod db;
mod error;
mod models;
mod routes;
mod services;

use axum::{Router, routing::{get, post, put, delete}};
use sqlx::SqlitePool;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use config::Config;
use crypto::CryptoService;
use models::AppMetrics;
use services::{FirmwareService, HmacService, GrayEngine, FailureMonitor};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Config,
    pub metrics: AppMetrics,
    pub firmware_service: FirmwareService,
    pub hmac_service: HmacService,
    pub gray_engine: GrayEngine,
    pub crypto_service: CryptoService,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("ota_server=info".parse()?)
        )
        .init();

    let config = Config::from_env();

    let pool = db::init_pool(&config.database_url).await?;
    db::run_migrations(&pool, "./migrations").await?;

    let metrics = AppMetrics::new();

    let firmware_service = FirmwareService::new(
        pool.clone(),
        std::path::PathBuf::from(&config.firmware_dir),
        config.rsa_private_key_pem.as_deref(),
        config.rsa_public_key_pem.as_deref(),
        metrics.clone(),
    )?;
    firmware_service.ensure_dir().await?;

    let hmac_service = HmacService::new(
        pool.clone(),
        config.hmac_primary_key.clone(),
        config.hmac_secondary_key.clone(),
        config.download_url_expiry_secs,
        metrics.clone(),
    );

    let gray_engine = GrayEngine::new(pool.clone(), metrics.clone());

    let crypto_service = CryptoService::new(&config.encryption_key);

    let (shutdown_flag, monitor_handle) = FailureMonitor::start_monitor_loop(
        pool.clone(),
        metrics.clone(),
    );

    let state = Arc::new(AppState {
        pool: pool.clone(),
        config: config.clone(),
        metrics: metrics.clone(),
        firmware_service,
        hmac_service,
        gray_engine,
        crypto_service,
    });

    let app = Router::new()
        .route("/api/v1/devices", post(routes::device::register_device))
        .route("/api/v1/devices/{device_id}", get(routes::device::get_device))
        .route("/api/v1/devices/{device_id}", put(routes::device::update_device))
        .route("/api/v1/devices/{device_id}/status", post(routes::device::transition_device_status))
        .route("/api/v1/devices/{device_id}/heartbeat", post(routes::device::heartbeat))
        .route("/api/v1/firmware", post(routes::firmware::upload_firmware))
        .route("/api/v1/firmware", get(routes::firmware::list_firmware))
        .route("/api/v1/firmware/{id}", get(routes::firmware::get_firmware))
        .route("/api/v1/firmware/{id}/activate", post(routes::firmware::activate_firmware))
        .route("/api/v1/firmware/{id}/deactivate", post(routes::firmware::deactivate_firmware))
        .route("/api/v1/firmware/{id}", delete(routes::firmware::delete_firmware))
        .route("/api/v1/firmware/{firmware_id}/download", get(routes::download::download_firmware))
        .route("/api/v1/tasks", post(routes::upgrade::create_task))
        .route("/api/v1/tasks", get(routes::upgrade::list_tasks))
        .route("/api/v1/tasks/{task_id}", get(routes::upgrade::get_task))
        .route("/api/v1/tasks/{task_id}/start", post(routes::upgrade::start_task))
        .route("/api/v1/tasks/{task_id}/advance", post(routes::upgrade::advance_task))
        .route("/api/v1/tasks/{task_id}/pause", post(routes::upgrade::pause_task))
        .route("/api/v1/tasks/{task_id}/resume", post(routes::upgrade::resume_task))
        .route("/api/v1/tasks/{task_id}/stats", get(routes::upgrade::get_task_stats))
        .route("/api/v1/tasks/{task_id}/devices", get(routes::upgrade::list_device_upgrades))
        .route("/api/v1/devices/{device_id}/check", post(routes::status::check_version))
        .route("/api/v1/devices/{device_id}/status", post(routes::status::report_status))
        .route("/api/v1/devices/{device_id}/status", get(routes::status::get_status_history))
        .route("/api/v1/audit", get(routes::audit::query_audit_logs))
        .route("/metrics", get(routes::metrics::metrics_endpoint))
        .route("/health", get(health_check))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("OTA Server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    {
        let mut shutdown = shutdown_flag.write().await;
        *shutdown = true;
    }
    let _ = monitor_handle.await;

    Ok(())
}

async fn health_check() -> &'static str {
    "ok"
}
