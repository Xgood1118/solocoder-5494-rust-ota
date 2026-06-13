use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use std::sync::Arc;

use crate::AppState;
use crate::error::AppResult;
use crate::models::{UpgradeTask, DeviceUpgrade, CreateUpgradeTaskRequest, Firmware};

#[derive(Serialize)]
pub struct TaskStats {
    pub total: i64,
    pub pending: i64,
    pub downloading: i64,
    pub installing: i64,
    pub success: i64,
    pub failed: i64,
    pub rebooted: i64,
    pub failure_rate: f64,
}

pub async fn create_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateUpgradeTaskRequest>,
) -> AppResult<Json<UpgradeTask>> {
    let fw = Firmware::find_by_id(&state.pool, req.firmware_id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(
            format!("Firmware {} not found", req.firmware_id)
        ))?;

    let task = UpgradeTask::create(&state.pool, &req, &fw.device_type).await?;

    crate::models::AuditLog::create(
        &state.pool,
        "task_create",
        "system",
        "upgrade_task",
        Some(&task.id.to_string()),
        None,
        Some(&serde_json::json!({
            "task_name": req.task_name,
            "firmware_id": req.firmware_id,
        }).to_string()),
        None,
        None,
    )
    .await?;

    Ok(Json(task))
}

pub async fn start_task(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    state.gray_engine.start_task(task_id).await?;
    Ok(Json(serde_json::json!({"status": "started"})))
}

pub async fn advance_task(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    let advanced = state.gray_engine.advance_stage(task_id).await?;
    Ok(Json(serde_json::json!({
        "status": if advanced { "advanced" } else { "completed" }
    })))
}

pub async fn pause_task(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    state.gray_engine.pause_task(task_id, "admin").await?;
    Ok(Json(serde_json::json!({"status": "paused"})))
}

pub async fn resume_task(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    state.gray_engine.resume_task(task_id, "admin").await?;
    Ok(Json(serde_json::json!({"status": "resumed"})))
}

pub async fn get_task(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<i64>,
) -> AppResult<Json<UpgradeTask>> {
    let task = UpgradeTask::find_by_id(&state.pool, task_id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(format!("Task {} not found", task_id)))?;
    Ok(Json(task))
}

pub async fn list_tasks(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<UpgradeTask>>> {
    let tasks = UpgradeTask::list_all(&state.pool).await?;
    Ok(Json(tasks))
}

pub async fn get_task_stats(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<i64>,
) -> AppResult<Json<TaskStats>> {
    let pool = &state.pool;
    let total = DeviceUpgrade::count_total(pool, task_id).await?;
    let pending = DeviceUpgrade::count_by_status(pool, task_id, "pending").await?;
    let downloading = DeviceUpgrade::count_by_status(pool, task_id, "downloading").await?;
    let installing = DeviceUpgrade::count_by_status(pool, task_id, "installing").await?;
    let success = DeviceUpgrade::count_by_status(pool, task_id, "success").await?;
    let failed = DeviceUpgrade::count_by_status(pool, task_id, "failed").await?;
    let rebooted = DeviceUpgrade::count_by_status(pool, task_id, "rebooted").await?;
    let failure_rate = DeviceUpgrade::calculate_failure_rate(pool, task_id).await?;

    Ok(Json(TaskStats {
        total,
        pending,
        downloading,
        installing,
        success,
        failed,
        rebooted,
        failure_rate,
    }))
}

pub async fn list_device_upgrades(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<i64>,
) -> AppResult<Json<Vec<DeviceUpgrade>>> {
    let list = DeviceUpgrade::list_by_task(&state.pool, task_id, 100, 0).await?;
    Ok(Json(list))
}
