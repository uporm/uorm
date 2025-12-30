use crate::Result;
use crate::error::DbError;
use crate::udbc::connection::Connection;
use crate::udbc::sqlite::value_codec::{from_sqlite_value, to_sqlite_value};
use crate::udbc::value::Value;
use async_trait::async_trait;
use rusqlite::params_from_iter;
use std::collections::HashMap;

/// Connection implementation for SQLite.
///
/// Wraps a `rusqlite::Connection` and executes queries in a blocking thread
/// to be compatible with async runtime (tokio).
pub struct SqliteConnection {
    /// The underlying SQLite connection.
    /// Wrapped in Option to allow moving it into the blocking task.
    conn: Option<rusqlite::Connection>,
}

impl SqliteConnection {
    pub fn new(conn: rusqlite::Connection) -> Self {
        Self { conn: Some(conn) }
    }

    /// Helper method to run a blocking closure with the database connection.
    ///
    /// This method handles the boilerplate of moving the connection into a `spawn_blocking` task
    /// and moving it back after execution.
    async fn run_blocking<F, T>(&mut self, f: F) -> Result<T>
    where
        F: FnOnce(&mut rusqlite::Connection) -> std::result::Result<T, rusqlite::Error>
            + Send
            + 'static,
        T: Send + 'static,
    {
        // Take the connection from the struct.
        // If it's None, it means the connection was lost (e.g., due to a previous panic).
        let conn = self
            .conn
            .take()
            .ok_or_else(|| DbError::DbError("Connection closed".to_string()))?;

        // Spawn a blocking task to run the database operation.
        let (conn, result): (rusqlite::Connection, std::result::Result<T, rusqlite::Error>) = tokio::task::spawn_blocking(move || -> (rusqlite::Connection, std::result::Result<T, rusqlite::Error>) {
            let mut conn = conn;
            let result = f(&mut conn);
            (conn, result)
        })
        .await
        .map_err(|e: tokio::task::JoinError| DbError::DbError(format!("Task failed: {}", e)))?;

        // Put the connection back.
        self.conn = Some(conn);

        // Return the result of the database operation.
        result.map_err(|e: rusqlite::Error| DbError::DbError(e.to_string()))
    }
}

#[async_trait]
impl Connection for SqliteConnection {
    async fn query(
        &mut self,
        sql: &str,
        args: &[(String, Value)],
    ) -> Result<Vec<HashMap<String, Value>>> {
        let sql = sql.to_string();
        // Convert arguments to SQLite values.
        let params = args
            .iter()
            .map(|(_, v)| to_sqlite_value(v))
            .collect::<Vec<_>>();

        self.run_blocking(move |conn| {
            let mut stmt = conn.prepare(&sql)?;
            let column_count = stmt.column_count();

            // pre-allocate column names to avoid repeated lookups
            let column_names: Vec<String> = (0..column_count)
                .map(|i| {
                    stmt.column_name(i)
                        .map(|s| s.to_string())
                        .unwrap_or_else(|_| i.to_string())
                })
                .collect();

            let mut rows = stmt.query(params_from_iter(params))?;
            let mut out = Vec::new();

            while let Some(row) = rows.next()? {
                let mut map = HashMap::with_capacity(column_count);
                for (i, name) in column_names.iter().enumerate() {
                    let v = row.get_ref(i)?;
                    map.insert(name.clone(), from_sqlite_value(v));
                }
                out.push(map);
            }
            Ok(out)
        })
        .await
    }

    async fn execute(&mut self, sql: &str, args: &[(String, Value)]) -> Result<u64> {
        let sql = sql.to_string();
        let params = args
            .iter()
            .map(|(_, v)| to_sqlite_value(v))
            .collect::<Vec<_>>();

        self.run_blocking(move |conn| {
            let count = conn.execute(&sql, params_from_iter(params))?;
            Ok(count as u64)
        })
        .await
    }

    async fn last_insert_id(&mut self) -> Result<u64> {
        self.run_blocking(|conn| {
            let id = conn.last_insert_rowid();
            // Ensure non-negative ID, though rowid is usually i64.
            Ok(id.max(0) as u64)
        })
        .await
    }

    async fn begin(&mut self) -> Result<()> {
        self.run_blocking(|conn| {
            conn.execute("BEGIN", [])?;
            Ok(())
        })
        .await
    }

    async fn commit(&mut self) -> Result<()> {
        self.run_blocking(|conn| {
            conn.execute("COMMIT", [])?;
            Ok(())
        })
        .await
    }

    async fn rollback(&mut self) -> Result<()> {
        self.run_blocking(|conn| {
            conn.execute("ROLLBACK", [])?;
            Ok(())
        })
        .await
    }
}
