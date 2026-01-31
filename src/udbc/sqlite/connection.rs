use crate::Result;
use crate::error::DbError;
use crate::udbc::connection::Connection;
use crate::udbc::sqlite::value_codec::{from_sqlite_value, to_sqlite_value};
use crate::udbc::value::Value;
use async_trait::async_trait;
use rusqlite::params_from_iter;
use std::collections::HashMap;

/// SQLite 的连接实现。
///
/// 包装 `rusqlite::Connection` 并在阻塞线程中执行查询，
/// 以兼容异步运行时（tokio）。
pub struct SqliteConnection {
    /// 底层 SQLite 连接。
    /// 使用 Option 包裹以便移入阻塞任务。
    conn: Option<rusqlite::Connection>,
}

impl SqliteConnection {
    pub fn new(conn: rusqlite::Connection) -> Self {
        Self { conn: Some(conn) }
    }

    /// 使用数据库连接执行阻塞闭包的辅助方法。
    ///
    /// 该方法负责将连接移入 `spawn_blocking` 任务并在执行后移回。
    async fn run_blocking<F, T>(&mut self, f: F) -> Result<T>
    where
        F: FnOnce(&mut rusqlite::Connection) -> std::result::Result<T, rusqlite::Error>
            + Send
            + 'static,
        T: Send + 'static,
    {
        // 从结构体中取出连接
        // 若为 None，表示连接已丢失（如之前发生 panic）
        let conn = self
            .conn
            .take()
            .ok_or_else(|| DbError::DbError("Connection closed".to_string()))?;

        // 启动阻塞任务执行数据库操作
        let (conn, result): (rusqlite::Connection, std::result::Result<T, rusqlite::Error>) = tokio::task::spawn_blocking(move || -> (rusqlite::Connection, std::result::Result<T, rusqlite::Error>) {
            let mut conn = conn;
            let result = f(&mut conn);
            (conn, result)
        })
        .await
        .map_err(|e: tokio::task::JoinError| DbError::DbError(format!("Task failed: {}", e)))?;

        // 放回连接
        self.conn = Some(conn);

        // 返回数据库操作结果
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
        // 将参数转换为 SQLite 值
        let params = args
            .iter()
            .map(|(_, v)| to_sqlite_value(v))
            .collect::<Vec<_>>();

        self.run_blocking(move |conn| {
            let mut stmt = conn.prepare(&sql)?;
            let column_count = stmt.column_count();

            // 预先分配列名，避免重复查询
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
            // 确保 ID 非负，尽管 rowid 通常为 i64
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
