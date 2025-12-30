use async_trait::async_trait;
use std::str::FromStr;
use std::time::Duration;

use crate::Result;
use crate::error::DbError;
use crate::udbc::connection::Connection;
use crate::udbc::driver::Driver;
use crate::udbc::sqlite::connection::SqliteConnection;
use crate::udbc::{DEFAULT_DB_NAME, PoolOptions};

const SQLITE_TYPE: &str = "sqlite";

#[derive(Debug, Clone)]
enum SqliteTarget {
    Memory,
    Path(String),
}

impl FromStr for SqliteTarget {
    type Err = DbError;

    fn from_str(url: &str) -> Result<Self> {
        let url = url.trim();
        // Support "sqlite://" or "sqlite:" prefix, or no prefix
        let path = if let Some(stripped) = url.strip_prefix("sqlite://") {
            stripped
        } else if let Some(stripped) = url.strip_prefix("sqlite:") {
            stripped
        } else {
            url
        };

        let path = path.trim();
        if path.is_empty() {
            return Err(DbError::DbUrlError(url.to_string()));
        }

        if path == ":memory:" {
            Ok(SqliteTarget::Memory)
        } else {
            Ok(SqliteTarget::Path(path.to_string()))
        }
    }
}

pub struct SqliteDriver {
    url: String,
    name: String,
    // type is constant "sqlite", no need to store it
    options: Option<PoolOptions>,
    target: Option<SqliteTarget>,
}

impl SqliteDriver {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            name: DEFAULT_DB_NAME.to_string(),
            url: url.into(),
            options: None,
            target: None,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn options(mut self, options: PoolOptions) -> Self {
        self.options = Some(options);
        self
    }

    pub fn build(mut self) -> Result<Self> {
        self.target = Some(SqliteTarget::from_str(&self.url)?);
        Ok(self)
    }

    fn open_connection(target: &SqliteTarget, timeout_secs: u64) -> Result<rusqlite::Connection> {
        let conn = match target {
            SqliteTarget::Memory => rusqlite::Connection::open_in_memory(),
            SqliteTarget::Path(p) => rusqlite::Connection::open(p),
        }
        .map_err(|e| DbError::DbError(format!("Failed to open connection: {}", e)))?;

        // Set busy_timeout FIRST to handle potential locks during PRAGMA execution
        if timeout_secs > 0 {
            conn.busy_timeout(Duration::from_secs(timeout_secs))
                .map_err(|e| DbError::DbError(format!("Failed to set busy_timeout: {}", e)))?;
        }

        // Enforce foreign keys for data integrity
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|e| DbError::DbError(format!("Failed to set foreign_keys: {}", e)))?;

        // WAL mode improves concurrency (readers don't block writers).
        // synchronous = NORMAL is safe for WAL and faster.
        // Note: Changing journal_mode requires a write lock on the database file.
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")
            .map_err(|e| DbError::DbError(format!("Failed to set journal_mode: {}", e)))?;

        Ok(conn)
    }
}

#[async_trait]
impl Driver for SqliteDriver {
    fn name(&self) -> &str {
        &self.name
    }

    fn r#type(&self) -> &str {
        SQLITE_TYPE
    }

    fn placeholder(&self, _param_seq: usize, _param_name: &str) -> String {
        "?".to_string()
    }

    async fn acquire(&self) -> Result<Box<dyn Connection>> {
        let target = self.target.as_ref().ok_or_else(|| {
            DbError::DbError(
                "Driver not built (target missing). Call build() after new().".to_string(),
            )
        })?;

        let target_clone = target.clone();
        let timeout_secs = self.options.as_ref().map(|o| o.timeout).unwrap_or(0);

        // SQLite operations are synchronous. Spawn a blocking task to avoid stalling the async runtime.
        // NOTE: This creates a new physical connection per call. For high throughput, a connection pool (e.g. r2d2) is recommended.
        // WARNING: For `SqliteTarget::Memory`, this creates a FRESH, empty database for every call.
        // To share in-memory state, use a file-based URL with shared cache (e.g. "file::memory:?cache=shared") and SqliteTarget::Path.
        let handle: tokio::task::JoinHandle<Result<Box<dyn Connection>>> =
            tokio::task::spawn_blocking(move || {
                let conn = Self::open_connection(&target_clone, timeout_secs)?;
                Ok::<Box<dyn Connection>, DbError>(
                    Box::new(SqliteConnection::new(conn)) as Box<dyn Connection>
                )
            });

        handle.await.map_err(|e: tokio::task::JoinError| {
            DbError::DbError(format!("Task join error: {}", e))
        })?
    }

    async fn close(&self) -> Result<()> {
        // No-op: connections are closed when dropped.
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
        let mut conn = driver.acquire().await.unwrap();

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
