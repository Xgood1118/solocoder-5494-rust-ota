use axum::{extract::State, response::IntoResponse};
use prometheus::Encoder;
use std::sync::Arc;

use crate::AppState;

pub async fn metrics_endpoint(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let encoder = prometheus::TextEncoder::new();
    let metric_families = state.metrics.registry.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        buffer,
    )
}
