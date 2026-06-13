use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditLog {
    pub id: i64,
    pub action: String,
    pub actor: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl AuditLog {
    pub async fn create(
        pool: &SqlitePool,
        action: &str,
        actor: &str,
        resource_type: &str,
        resource_id: Option<&str>,
        old_value: Option<&str>,
        new_value: Option<&str>,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> AppResult<Self> {
        let now = Utc::now();
        sqlx::query(
            r#"INSERT INTO audit_logs (action, actor, resource_type, resource_id,
                                        old_value, new_value, ip_address, user_agent, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#
        )
        .bind(action)
        .bind(actor)
        .bind(resource_type)
        .bind(resource_id)
        .bind(old_value)
        .bind(new_value)
        .bind(ip_address)
        .bind(user_agent)
        .bind(now)
        .execute(pool)
        .await?;

        let id: i64 = sqlx::query_scalar("SELECT last_insert_rowid()")
            .fetch_one(pool)
            .await?;

        let log = sqlx::query_as::<_, AuditLog>(
            r#"SELECT id, action, actor, resource_type, resource_id, old_value, new_value,
                      ip_address, user_agent, created_at
               FROM audit_logs WHERE id = ?"#
        )
        .bind(id)
        .fetch_one(pool)
        .await?;

        Ok(log)
    }

    pub async fn query(pool: &SqlitePool, q: &AuditLogQuery) -> AppResult<Vec<Self>> {
        let limit = q.limit.unwrap_or(50);
        let offset = q.offset.unwrap_or(0);

        let logs = sqlx::query_as::<_, AuditLog>(
            r#"SELECT id, action, actor, resource_type, resource_id, old_value, new_value,
                      ip_address, user_agent, created_at
               FROM audit_logs
               WHERE (? IS NULL OR action = ?)
                 AND (? IS NULL OR resource_type = ?)
                 AND (? IS NULL OR resource_id = ?)
               ORDER BY id DESC LIMIT ? OFFSET ?"#
        )
        .bind(&q.action)
        .bind(&q.action)
        .bind(&q.resource_type)
        .bind(&q.resource_type)
        .bind(&q.resource_id)
        .bind(&q.resource_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(logs)
    }
}
