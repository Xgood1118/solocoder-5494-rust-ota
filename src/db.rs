use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use tokio::fs;

use crate::error::AppResult;

pub async fn init_pool(database_url: &str) -> AppResult<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await?;

    sqlx::query("PRAGMA journal_mode = WAL;")
        .execute(&pool)
        .await?;

    sqlx::query("PRAGMA synchronous = NORMAL;")
        .execute(&pool)
        .await?;

    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(&pool)
        .await?;

    Ok(pool)
}

pub async fn run_migrations(pool: &SqlitePool, migrations_dir: &str) -> AppResult<()> {
    let mut entries = fs::read_dir(migrations_dir).await?;
    let mut migration_files = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().map(|e| e == "sql").unwrap_or(false) {
            migration_files.push(path);
        }
    }

    migration_files.sort();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS __migrations (
            name TEXT PRIMARY KEY,
            applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(pool)
    .await?;

    for path in migration_files {
        let name = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let applied: Option<String> = sqlx::query_scalar(
            "SELECT name FROM __migrations WHERE name = ?"
        )
        .bind(&name)
        .fetch_optional(pool)
        .await?;

        if applied.is_none() {
            let sql = fs::read_to_string(&path).await?;
            let mut tx = pool.begin().await?;

            for statement in sql.split(';') {
                let stmt = statement.trim();
                if !stmt.is_empty() {
                    sqlx::query(stmt).execute(&mut *tx).await?;
                }
            }

            sqlx::query("INSERT INTO __migrations (name) VALUES (?)")
                .bind(&name)
                .execute(&mut *tx)
                .await?;

            tx.commit().await?;

            tracing::info!("Applied migration: {}", name);
        }
    }

    Ok(())
}
