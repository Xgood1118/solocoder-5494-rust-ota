use prometheus::{IntCounter, IntGauge, GaugeVec, Histogram, Registry, histogram_opts, Opts};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct AppMetrics {
    pub registry: Arc<Registry>,
    pub firmware_uploads: IntCounter,
    pub firmware_downloads: IntCounter,
    pub download_bytes: IntCounter,
    pub active_tasks: IntGauge,
    pub devices_registered: IntGauge,
    pub upgrade_success: IntCounter,
    pub upgrade_failed: IntCounter,
    pub upgrade_downloading: IntGauge,
    pub upgrade_installing: IntGauge,
    pub hmac_rotations: IntCounter,
    pub download_duration: Histogram,
    pub status_reports: IntCounter,
    pub task_failure_rate: GaugeVec,
    pub auto_paused_tasks: IntGauge,
}

impl AppMetrics {
    pub fn new() -> Self {
        let registry = Arc::new(Registry::new());

        let firmware_uploads = IntCounter::new("ota_firmware_uploads_total", "Total firmware uploads")
            .unwrap();
        let firmware_downloads = IntCounter::new("ota_firmware_downloads_total", "Total firmware downloads")
            .unwrap();
        let download_bytes = IntCounter::new("ota_download_bytes_total", "Total bytes downloaded")
            .unwrap();
        let active_tasks = IntGauge::new("ota_active_tasks", "Currently active upgrade tasks")
            .unwrap();
        let devices_registered = IntGauge::new("ota_devices_registered", "Total registered devices")
            .unwrap();
        let upgrade_success = IntCounter::new("ota_upgrade_success_total", "Total successful upgrades")
            .unwrap();
        let upgrade_failed = IntCounter::new("ota_upgrade_failed_total", "Total failed upgrades")
            .unwrap();
        let upgrade_downloading = IntGauge::new("ota_upgrade_downloading", "Devices currently downloading")
            .unwrap();
        let upgrade_installing = IntGauge::new("ota_upgrade_installing", "Devices currently installing")
            .unwrap();
        let hmac_rotations = IntCounter::new("ota_hmac_key_rotations_total", "Total HMAC key rotations")
            .unwrap();
        let download_duration = Histogram::with_opts(
            histogram_opts!(
                "ota_download_duration_seconds",
                "Download duration in seconds",
                vec![0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0]
            )
        ).unwrap();
        let status_reports = IntCounter::new("ota_status_reports_total", "Total status reports received")
            .unwrap();
        let task_failure_rate = GaugeVec::new(
            Opts::new("ota_task_failure_rate", "Failure rate per upgrade task"),
            &["task_id", "task_name"]
        ).unwrap();
        let auto_paused_tasks = IntGauge::new("ota_auto_paused_tasks", "Tasks auto-paused due to high failure rate")
            .unwrap();

        registry.register(Box::new(firmware_uploads.clone())).unwrap();
        registry.register(Box::new(firmware_downloads.clone())).unwrap();
        registry.register(Box::new(download_bytes.clone())).unwrap();
        registry.register(Box::new(active_tasks.clone())).unwrap();
        registry.register(Box::new(devices_registered.clone())).unwrap();
        registry.register(Box::new(upgrade_success.clone())).unwrap();
        registry.register(Box::new(upgrade_failed.clone())).unwrap();
        registry.register(Box::new(upgrade_downloading.clone())).unwrap();
        registry.register(Box::new(upgrade_installing.clone())).unwrap();
        registry.register(Box::new(hmac_rotations.clone())).unwrap();
        registry.register(Box::new(download_duration.clone())).unwrap();
        registry.register(Box::new(status_reports.clone())).unwrap();
        registry.register(Box::new(task_failure_rate.clone())).unwrap();
        registry.register(Box::new(auto_paused_tasks.clone())).unwrap();

        Self {
            registry,
            firmware_uploads,
            firmware_downloads,
            download_bytes,
            active_tasks,
            devices_registered,
            upgrade_success,
            upgrade_failed,
            upgrade_downloading,
            upgrade_installing,
            hmac_rotations,
            download_duration,
            status_reports,
            task_failure_rate,
            auto_paused_tasks,
        }
    }
}
