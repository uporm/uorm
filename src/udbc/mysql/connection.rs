use async_trait::async_trait;
use mysql_async::prelude::Queryable;
use mysql_async::{Conn, Row as MyRow};
use std::collections::HashMap;

use crate::error::DbError;
use crate::Result;
use crate::udbc::connection::Connection;
use crate::udbc::mysql::value_codec::{from_mysql_value, to_mysql_value};
use crate::udbc::value::Value;

pub struct MysqlConnection {
    conn: Conn,
}

impl MysqlConnection {
    pub fn new(conn: Conn) -> Self {
        Self { conn }
    }

    // Optimize: consume row to avoid cloning values, use columns() to avoid intermediate Vec allocation
    fn map_row(row: MyRow) -> HashMap<String, Value> {
        // Access column metadata via Arc (cheap)
        let columns = row.columns();
        // Consume row to get values (moves ownership, efficient)
        let values = row.unwrap();

        let mut out_row = HashMap::with_capacity(values.len());
        // Zip values with columns. We rely on the driver ensuring lengths match.
        for (v, col) in values.into_iter().zip(columns.iter()) {
            out_row.insert(col.name_str().to_string(), from_mysql_value(v));
        }
        out_row
    }
}

#[async_trait]
impl Connection for MysqlConnection {
    async fn query(
        &mut self,
        sql: &str,
        args: &[(String, Value)],
    ) -> Result<Vec<HashMap<String, Value>>> {
        // Map args to positional params. Note: We ignore keys in args as mysql_async
        // expects Positional params for '?' placeholders.
        let params =
            mysql_async::Params::Positional(args.iter().map(|(_, v)| to_mysql_value(v)).collect());

        let rows: Vec<MyRow> = self.conn.exec(sql, params).await.map_err(|e| {
            DbError::DbError(e.to_string())
        })?;
        Ok(rows.into_iter().map(Self::map_row).collect())
    }

    async fn execute(&mut self, sql: &str, args: &[(String, Value)]) -> Result<u64> {
        let params =
            mysql_async::Params::Positional(args.iter().map(|(_, v)| to_mysql_value(v)).collect());

        self.conn.exec_drop(sql, params).await.map_err(|e| {
            DbError::DbError(e.to_string())
        })?;
        Ok(self.conn.affected_rows())
    }

    async fn last_insert_id(&mut self) -> Result<u64> {
        // unwrap_or(0) handles cases where no insert happened or ID is unavailable
        Ok(self.conn.last_insert_id().unwrap_or(0))
    }

    async fn begin(&mut self) -> Result<()> {
        self.conn.query_drop("BEGIN").await.map_err(|e| {
            DbError::DbError(e.to_string())
        })?;
        Ok(())
    }

    async fn commit(&mut self) -> Result<()> {
        self.conn.query_drop("COMMIT").await.map_err(|e| {
            DbError::DbError(e.to_string())
        })?;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<()> {
        self.conn.query_drop("ROLLBACK").await.map_err(|e| {
            DbError::DbError(e.to_string())
        })?;
        Ok(())
    }
}
