use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, interval};

use crate::error::AppResult;
use crate::models::{UpgradeTask, DeviceUpgrade, AuditLog, AppMetrics};

#[derive(Clone)]
pub struct FailureMonitor {
    pub pool: SqlitePool,
    pub metrics: AppMetrics,
}

impl FailureMonitor {
    pub fn new(pool: SqlitePool, metrics: AppMetrics) -> Self {
        Self { pool, metrics }
    }

    pub async fn check_and_auto_pause(&self) -> AppResult<Vec<i64>> {
        let tasks = UpgradeTask::list_all_active(&self.pool).await?;
        let mut paused = Vec::new();
        let mut auto_paused_count = 0i64;

        for task in &tasks {
            let failure_rate = DeviceUpgrade::calculate_failure_rate(&self.pool, task.id).await?;

            self.update_metrics(task, failure_rate);

            if task.auto_paused {
                auto_paused_count += 1;
            }

            if task.status != "running" || task.auto_paused {
                continue;
            }

            if failure_rate > task.failure_rate_threshold {
                tracing::warn!(
                    "Task {} failure rate {:.2}% exceeds threshold {:.2}%, auto-pausing",
                    task.id,
                    failure_rate * 100.0,
                    task.failure_rate_threshold * 100.0,
                );

                UpgradeTask::set_auto_paused(&self.pool, task.id, true).await?;
                paused.push(task.id);
                auto_paused_count += 1;

                AuditLog::create(
                    &self.pool,
                    "task_auto_paused",
                    "failure_monitor",
                    "upgrade_task",
                    Some(&task.id.to_string()),
                    None,
                    Some(&serde_json::json!({
                        "failure_rate": failure_rate,
                        "threshold": task.failure_rate_threshold,
                    }).to_string()),
                    None,
                    None,
                )
                .await?;
            }
        }

        self.metrics.active_tasks.set(tasks.len() as i64);
        self.metrics.auto_paused_tasks.set(auto_paused_count);

        Ok(paused)
    }

    fn update_metrics(&self, task: &UpgradeTask, failure_rate: f64) {
        self.metrics
            .task_failure_rate
            .with_label_values(&[&task.id.to_string(), &task.task_name])
            .set(failure_rate);
    }

    pub fn start_monitor_loop(pool: SqlitePool, metrics: AppMetrics) -> (Arc<RwLock<bool>>, tokio::task::JoinHandle<()>) {
        let shutdown = Arc::new(RwLock::new(false));
        let shutdown_clone = shutdown.clone();
        let monitor = Self::new(pool, metrics);

        let handle = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(30));
            loop {
                ticker.tick().await;

                if *shutdown_clone.read().await {
                    break;
                }

                if let Err(e) = monitor.check_and_auto_pause().await {
                    tracing::error!("Failure monitor error: {}", e);
                }
            }
        });

        (shutdown, handle)
    }
}
