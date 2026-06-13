use sqlx::SqlitePool;

use crate::error::AppResult;
use crate::models::AuditLog;

#[derive(Clone)]
pub struct AuditService {
    pub pool: SqlitePool,
}

impl AuditService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn log(
        &self,
        action: &str,
        actor: &str,
        resource_type: &str,
        resource_id: Option<&str>,
        old_value: Option<&str>,
        new_value: Option<&str>,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> AppResult<AuditLog> {
        AuditLog::create(
            &self.pool,
            action,
            actor,
            resource_type,
            resource_id,
            old_value,
            new_value,
            ip_address,
            user_agent,
        )
        .await
    }
}
