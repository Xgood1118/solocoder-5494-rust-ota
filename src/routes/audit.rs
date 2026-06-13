use axum::{
    extract::{Query, State},
    Json,
};
use std::sync::Arc;

use crate::AppState;
use crate::error::AppResult;
use crate::models::AuditLogQuery;

pub async fn query_audit_logs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuditLogQuery>,
) -> AppResult<Json<Vec<crate::models::AuditLog>>> {
    let logs = crate::models::AuditLog::query(&state.pool, &query).await?;
    Ok(Json(logs))
}
