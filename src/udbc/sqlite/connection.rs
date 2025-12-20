use async_trait::async_trait;
use rusqlite::params_from_iter;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::error::DbError;
use crate::udbc::connection::Connection;
use crate::udbc::sqlite::value_codec::{from_sqlite_value, to_sqlite_value};
use crate::udbc::value::Value;

pub struct SqliteConnection {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteConnection {
    pub fn new(conn: rusqlite::Connection) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
        }
    }
}

#[async_trait]
impl Connection for SqliteConnection {
    async fn query(
        &self,
        sql: &str,
        args: &[(String, Value)],
    ) -> Result<Vec<HashMap<String, Value>>, DbError> {
        let sql = sql.to_string();
        let params = args
            .iter()
            .map(|(_, v)| to_sqlite_value(v))
            .collect::<Vec<_>>();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let mut stmt = conn.prepare(&sql)?;
            let column_count = stmt.column_count();
            let column_names = (0..column_count)
                .map(|i| {
                    stmt.column_name(i)
                        .map(|s| s.to_string())
                        .unwrap_or_else(|_| i.to_string())
                })
                .collect::<Vec<_>>();

            let mut rows = stmt.query(params_from_iter(params))?;
            let mut out = Vec::new();

            while let Some(row) = rows.next()? {
                let mut map = HashMap::with_capacity(column_count);
                for i in 0..column_count {
                    let name = column_names
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| i.to_string());
                    let v = row.get_ref(i)?;
                    map.insert(name, from_sqlite_value(v));
                }
                out.push(map);
            }

            Ok::<_, DbError>(out)
        })
        .await
        .map_err(|e| DbError::Database(e.to_string()))?
    }

    async fn execute(&self, sql: &str, args: &[(String, Value)]) -> Result<u64, DbError> {
        let sql = sql.to_string();
        let params = args
            .iter()
            .map(|(_, v)| to_sqlite_value(v))
            .collect::<Vec<_>>();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let affected = conn.execute(&sql, params_from_iter(params))?;
            Ok::<_, DbError>(affected as u64)
        })
        .await
        .map_err(|e| DbError::Database(e.to_string()))?
    }

    async fn last_insert_id(&self) -> Result<u64, DbError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            Ok::<_, DbError>(conn.last_insert_rowid().max(0) as u64)
        })
        .await
        .map_err(|e| DbError::Database(e.to_string()))?
    }

    async fn begin(&self) -> Result<(), DbError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            conn.execute_batch("BEGIN")?;
            Ok::<_, DbError>(())
        })
        .await
        .map_err(|e| DbError::Database(e.to_string()))?
    }

    async fn commit(&self) -> Result<(), DbError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            conn.execute_batch("COMMIT")?;
            Ok::<_, DbError>(())
        })
        .await
        .map_err(|e| DbError::Database(e.to_string()))?
    }

    async fn rollback(&self) -> Result<(), DbError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            conn.execute_batch("ROLLBACK")?;
            Ok::<_, DbError>(())
        })
        .await
        .map_err(|e| DbError::Database(e.to_string()))?
    }
}
