use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

use crate::error::DbError;
use crate::udbc::connection::Connection;
use crate::udbc::driver::Driver;
use crate::udbc::sqlite::connection::SqliteConnection;
use crate::udbc::{ConnectionOptions, DEFAULT_DB_NAME};

const SQLITE_TYPE: &str = "sqlite";

enum SqliteTarget {
    Memory,
    Path(String),
}

pub struct SqliteDriver {
    url: String,
    name: String,
    r#type: String,
    options: Option<ConnectionOptions>,
    target: Option<SqliteTarget>,
}

impl SqliteDriver {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            name: DEFAULT_DB_NAME.to_string(),
            r#type: SQLITE_TYPE.to_string(),
            url: url.into(),
            options: None,
            target: None,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn options(mut self, options: ConnectionOptions) {
        self.options = Some(options);
    }

    fn parse_target(url: &str) -> Result<SqliteTarget, DbError> {
        let trimmed = url.trim();
        let stripped = trimmed
            .strip_prefix("sqlite://")
            .or_else(|| trimmed.strip_prefix("sqlite:"))
            .unwrap_or(trimmed)
            .trim();

        if stripped.is_empty() {
            return Err(DbError::InvalidDatabaseUrl(url.to_string()));
        }

        if stripped == ":memory:" {
            return Ok(SqliteTarget::Memory);
        }

        Ok(SqliteTarget::Path(stripped.to_string()))
    }

    pub fn build(mut self) -> Result<Self, DbError> {
        let target = Self::parse_target(&self.url)?;
        self.target = Some(target);
        Ok(self)
    }

    fn open_connection(
        target: &SqliteTarget,
        timeout_secs: u64,
    ) -> Result<rusqlite::Connection, DbError> {
        let conn = match target {
            SqliteTarget::Memory => rusqlite::Connection::open_in_memory()?,
            SqliteTarget::Path(p) => rusqlite::Connection::open(p)?,
        };

        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        if timeout_secs > 0 {
            conn.busy_timeout(Duration::from_secs(timeout_secs))?;
        }

        Ok(conn)
    }
}

#[async_trait]
impl Driver for SqliteDriver {
    fn name(&self) -> &str {
        &self.name
    }

    fn r#type(&self) -> &str {
        &self.r#type
    }

    fn placeholder(&self, _param_seq: usize, _param_name: &str) -> String {
        "?".to_string()
    }

    async fn connection(&self) -> Result<Arc<dyn Connection>, DbError> {
        let target = self
            .target
            .as_ref()
            .ok_or_else(|| DbError::Database("Target not initialized".to_string()))?;
        let url_target = match target {
            SqliteTarget::Memory => SqliteTarget::Memory,
            SqliteTarget::Path(p) => SqliteTarget::Path(p.clone()),
        };
        let timeout_secs = self.options.as_ref().map(|o| o.timeout).unwrap_or(0);

        tokio::task::spawn_blocking(move || {
            let conn = Self::open_connection(&url_target, timeout_secs)?;
            Ok::<_, DbError>(Arc::new(SqliteConnection::new(conn)) as Arc<dyn Connection>)
        })
        .await
        .map_err(|e| DbError::Database(e.to_string()))?
    }

    async fn close(&self) -> Result<(), DbError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::udbc::value::Value;

    #[tokio::test]
    async fn test_sqlite_driver_in_memory() {
        let driver = SqliteDriver::new("sqlite::memory:").build().unwrap();
        let conn = driver.connection().await.unwrap();

        conn.execute(
            "CREATE TABLE user (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
            &[],
        )
        .await
        .unwrap();

        conn.execute(
            "INSERT INTO user(name) VALUES (?)",
            &[("name".to_string(), Value::Str("alice".to_string()))],
        )
        .await
        .unwrap();

        let id = conn.last_insert_id().await.unwrap();
        assert!(id > 0);

        let rows = conn
            .query(
                "SELECT id, name FROM user WHERE id = ?",
                &[("id".to_string(), Value::I64(id as i64))],
            )
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("name"), Some(&Value::Str("alice".to_string())));
    }
}
