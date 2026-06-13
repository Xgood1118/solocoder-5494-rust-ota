use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header, HeaderName},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::sync::Arc;

use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::models::Firmware;

#[derive(Deserialize)]
pub struct DownloadQuery {
    pub device_id: String,
    pub expires: u64,
    pub key_id: String,
    pub sig: String,
}

static X_SHA256: HeaderName = HeaderName::from_static("x-sha256");
static X_RSA_SIGNATURE: HeaderName = HeaderName::from_static("x-rsa-signature");

pub async fn download_firmware(
    State(state): State<Arc<AppState>>,
    Path(firmware_id): Path<i64>,
    Query(query): Query<DownloadQuery>,
    headers: HeaderMap,
) -> AppResult<Response> {
    state.hmac_service.verify_download_signature(
        firmware_id,
        &query.device_id,
        query.expires,
        &query.key_id,
        &query.sig,
    )?;

    let fw = Firmware::find_by_id(&state.pool, firmware_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Firmware {} not found", firmware_id)))?;

    let file_size = state.firmware_service.get_file_size(&fw.file_path).await?;

    if let Some(range_header) = headers.get(header::RANGE) {
        let range_str = range_header.to_str().map_err(|_| {
            AppError::BadRequest("Invalid Range header".to_string())
        })?;

        let (start, end) = parse_range(range_str, file_size)?;

        let data = state.firmware_service.read_file_range(&fw.file_path, start, end).await?;

        state.metrics.firmware_downloads.inc();
        state.metrics.download_bytes.inc_by(data.len() as u64);

        let content_range = format!("bytes {}-{}/{}", start, end, file_size);

        let mut response_headers = HeaderMap::new();
        response_headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));
        response_headers.insert(header::CONTENT_LENGTH, HeaderValue::from(data.len() as u64));
        response_headers.insert(header::CONTENT_RANGE, HeaderValue::try_from(content_range.as_str()).unwrap());
        response_headers.insert(&X_SHA256, HeaderValue::try_from(fw.sha256_hash.as_str()).unwrap());
        response_headers.insert(&X_RSA_SIGNATURE, HeaderValue::try_from(fw.rsa_signature.as_str()).unwrap());

        Ok((
            StatusCode::PARTIAL_CONTENT,
            response_headers,
            Body::from(data),
        )
            .into_response())
    } else {
        let data = state.firmware_service.read_file(&fw.file_path).await?;

        state.metrics.firmware_downloads.inc();
        state.metrics.download_bytes.inc_by(data.len() as u64);

        let mut response_headers = HeaderMap::new();
        response_headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));
        response_headers.insert(header::CONTENT_LENGTH, HeaderValue::from(data.len() as u64));
        response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        response_headers.insert(&X_SHA256, HeaderValue::try_from(fw.sha256_hash.as_str()).unwrap());
        response_headers.insert(&X_RSA_SIGNATURE, HeaderValue::try_from(fw.rsa_signature.as_str()).unwrap());

        Ok((
            StatusCode::OK,
            response_headers,
            Body::from(data),
        )
            .into_response())
    }
}

fn parse_range(range_str: &str, file_size: u64) -> AppResult<(u64, u64)> {
    let range = range_str.strip_prefix("bytes=").ok_or_else(|| {
        AppError::BadRequest("Invalid Range format, expected bytes= prefix".to_string())
    })?;

    let parts: Vec<&str> = range.split('-').collect();
    if parts.len() != 2 {
        return Err(AppError::BadRequest("Invalid Range format".to_string()));
    }

    let start = if parts[0].is_empty() {
        0
    } else {
        parts[0].parse::<u64>().map_err(|_| {
            AppError::BadRequest("Invalid range start".to_string())
        })?
    };

    let end = if parts[1].is_empty() {
        file_size - 1
    } else {
        let e = parts[1].parse::<u64>().map_err(|_| {
            AppError::BadRequest("Invalid range end".to_string())
        })?;
        e.min(file_size - 1)
    };

    if start > end {
        return Err(AppError::BadRequest("Invalid range: start > end".to_string()));
    }

    Ok((start, end))
}
