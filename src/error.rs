use axum::{
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("URL expired")]
    UrlExpired,

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("RSA error: {0}")]
    Rsa(String),

    #[error("Multipart error: {0}")]
    Multipart(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_type, message) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "unauthorized", msg),
            AppError::InvalidSignature(msg) => (StatusCode::FORBIDDEN, "invalid_signature", msg),
            AppError::UrlExpired => (StatusCode::FORBIDDEN, "url_expired", "URL has expired".to_string()),
            AppError::Database(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                e.to_string(),
            ),
            AppError::Io(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "io_error",
                e.to_string(),
            ),
            AppError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                msg,
            ),
            AppError::Json(e) => (
                StatusCode::BAD_REQUEST,
                "json_error",
                e.to_string(),
            ),
            AppError::Rsa(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "rsa_error",
                msg,
            ),
            AppError::Multipart(msg) => (
                StatusCode::BAD_REQUEST,
                "multipart_error",
                msg,
            ),
        };

        (
            status,
            Json(ErrorResponse {
                error: error_type.to_string(),
                message,
            }),
        )
            .into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
