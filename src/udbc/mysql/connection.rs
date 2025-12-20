use async_trait::async_trait;
use mysql_async::prelude::Queryable;
use mysql_async::{Conn, Row as MyRow};
use std::collections::HashMap;
use tokio::sync::Mutex;

use crate::error::DbError;
use crate::udbc::connection::Connection;
use crate::udbc::mysql::value_codec::{from_mysql_value, to_mysql_value};
use crate::udbc::value::Value;

pub struct MysqlConnection {
    conn: Mutex<Conn>,
}

impl MysqlConnection {
    pub fn new(conn: Conn) -> Self {
        Self {
            conn: Mutex::new(conn),
        }
    }

    fn map_row(row: MyRow) -> HashMap<String, Value> {
        let mut out = HashMap::new();
        let cols = row.columns_ref();
        let len = row.len();
        for i in 0..len {
            let v = row.as_ref(i).expect("value");
            let name = cols
                .get(i)
                .map(|c| c.name_str().to_string())
                .unwrap_or_else(|| i.to_string());
            out.insert(name, from_mysql_value(v));
        }
        out
    }
}

#[async_trait]
impl Connection for MysqlConnection {
    async fn query(
        &self,
        sql: &str,
        args: &[(String, Value)],
    ) -> Result<Vec<HashMap<String, Value>>, DbError> {
        let mut conn = self.conn.lock().await;
        let params =
            mysql_async::Params::Positional(args.iter().map(|(_, v)| to_mysql_value(v)).collect());
        let rows: Vec<MyRow> = conn.exec(sql, params).await?;
        Ok(rows.into_iter().map(Self::map_row).collect())
    }

    async fn execute(&self, sql: &str, args: &[(String, Value)]) -> Result<u64, DbError> {
        let mut conn = self.conn.lock().await;
        let params =
            mysql_async::Params::Positional(args.iter().map(|(_, v)| to_mysql_value(v)).collect());
        conn.exec_drop(sql, params).await?;
        Ok(conn.affected_rows())
    }

    async fn last_insert_id(&self) -> Result<u64, DbError> {
        let conn = self.conn.lock().await;
        Ok(conn.last_insert_id().unwrap_or(0))
    }

    async fn begin(&self) -> Result<(), DbError> {
        self.conn.lock().await.query_drop("BEGIN").await?;
        Ok(())
    }

    async fn commit(&self) -> Result<(), DbError> {
        self.conn.lock().await.query_drop("COMMIT").await?;
        Ok(())
    }

    async fn rollback(&self) -> Result<(), DbError> {
        self.conn.lock().await.query_drop("ROLLBACK").await?;
        Ok(())
    }
}
