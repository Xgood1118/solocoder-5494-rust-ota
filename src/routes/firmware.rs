use axum::{
    extract::{Multipart, Path, State, Query},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::AppState;
use crate::error::AppResult;
use crate::models::{Firmware, FirmwareResponse};

#[derive(Deserialize)]
pub struct ListFirmwareQuery {
    device_type: String,
}

pub async fn upload_firmware(
    State(state): State<Arc<AppState>>,
    multipart: Multipart,
) -> AppResult<Json<FirmwareResponse>> {
    let mut version = String::new();
    let mut device_type = String::new();
    let mut description: Option<String> = None;
    let mut metadata: Option<String> = None;
    let mut firmware_data: Option<Vec<u8>> = None;

    let mut multipart = multipart;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        crate::error::AppError::Multipart(format!("Multipart error: {}", e))
    })? {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                firmware_data = Some(field.bytes().await.map_err(|e| {
                    crate::error::AppError::Multipart(format!("Read file error: {}", e))
                })?.to_vec());
            }
            "version" => {
                version = field.text().await.map_err(|e| {
                    crate::error::AppError::Multipart(format!("Read version error: {}", e))
                })?;
            }
            "device_type" => {
                device_type = field.text().await.map_err(|e| {
                    crate::error::AppError::Multipart(format!("Read device_type error: {}", e))
                })?;
            }
            "description" => {
                description = Some(field.text().await.map_err(|e| {
                    crate::error::AppError::Multipart(format!("Read description error: {}", e))
                })?);
            }
            "metadata" => {
                metadata = Some(field.text().await.map_err(|e| {
                    crate::error::AppError::Multipart(format!("Read metadata error: {}", e))
                })?);
            }
            _ => {}
        }
    }

    let data = firmware_data.ok_or_else(|| {
        crate::error::AppError::BadRequest("Missing file field".to_string())
    })?;

    if version.is_empty() {
        return Err(crate::error::AppError::BadRequest("Missing version field".to_string()));
    }
    if device_type.is_empty() {
        return Err(crate::error::AppError::BadRequest("Missing device_type field".to_string()));
    }

    let fw = state.firmware_service
        .upload(
            &version,
            &device_type,
            &data,
            description.as_deref(),
            metadata.as_deref(),
        )
        .await?;

    Ok(Json(fw.into()))
}

pub async fn list_firmware(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListFirmwareQuery>,
) -> AppResult<Json<Vec<FirmwareResponse>>> {
    let list = Firmware::list_by_device_type(&state.pool, &query.device_type).await?;
    Ok(Json(list.into_iter().map(|f| f.into()).collect()))
}

pub async fn get_firmware(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> AppResult<Json<FirmwareResponse>> {
    let fw = Firmware::find_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(format!("Firmware {} not found", id)))?;
    Ok(Json(fw.into()))
}

pub async fn activate_firmware(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    Firmware::set_active(&state.pool, id, true).await?;

    crate::models::AuditLog::create(
        &state.pool,
        "firmware_activate",
        "system",
        "firmware",
        Some(&id.to_string()),
        Some("inactive"),
        Some("active"),
        None,
        None,
    )
    .await?;

    Ok(Json(serde_json::json!({"status": "activated"})))
}

pub async fn deactivate_firmware(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    Firmware::set_active(&state.pool, id, false).await?;

    crate::models::AuditLog::create(
        &state.pool,
        "firmware_deactivate",
        "system",
        "firmware",
        Some(&id.to_string()),
        Some("active"),
        Some("inactive"),
        None,
        None,
    )
    .await?;

    Ok(Json(serde_json::json!({"status": "deactivated"})))
}

pub async fn delete_firmware(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    state.firmware_service.delete_firmware(id).await?;
    Ok(Json(serde_json::json!({"status": "deleted"})))
}
