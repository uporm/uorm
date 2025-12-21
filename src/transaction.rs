use crate::error::DbError;
use crate::tpl::engine;
use crate::udbc::connection::Connection;
use crate::udbc::driver::Driver;
use crate::udbc::value::Value;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

pub struct TransactionContext {
    conn: Option<Box<dyn Connection>>,
    committed: bool,
    driver: Arc<dyn Driver>,
}

impl TransactionContext {
    pub async fn begin(pool: Arc<dyn Driver>) -> Result<Self, DbError> {
        let mut conn = pool.acquire().await?;
        conn.begin().await?;
        Ok(Self {
            conn: Some(conn),
            committed: false,
            driver: pool,
        })
    }

    pub async fn commit(&mut self) -> Result<(), DbError> {
        if let Some(conn) = self.conn.as_mut() {
            conn.commit().await?;
        }
        self.committed = true;
        Ok(())
    }

    pub async fn rollback(&mut self) -> Result<(), DbError> {
        let r = if let Some(conn) = self.conn.as_mut() {
            conn.rollback().await
        } else {
            Ok(())
        };
        if r.is_ok() {
            self.committed = true;
        }
        r
    }

    pub async fn query<T: Serialize>(
        &mut self,
        sql: &str,
        args: &T,
    ) -> Result<Vec<HashMap<String, Value>>, DbError> {
        let (rendered_sql, params) = engine::render_template(sql, sql, args, self.driver.as_ref())?;
        if let Some(conn) = self.conn.as_mut() {
            conn.query(&rendered_sql, &params).await
        } else {
            Err(DbError::Database("Connection closed".to_string()))
        }
    }

    pub async fn execute<T: Serialize>(&mut self, sql: &str, args: &T) -> Result<u64, DbError> {
        let (rendered_sql, params) = engine::render_template(sql, sql, args, self.driver.as_ref())?;
        if let Some(conn) = self.conn.as_mut() {
            conn.execute(&rendered_sql, &params).await
        } else {
            Err(DbError::Database("Connection closed".to_string()))
        }
    }

    pub async fn last_insert_id(&mut self) -> Result<u64, DbError> {
        if let Some(conn) = self.conn.as_mut() {
            conn.last_insert_id().await
        } else {
            Err(DbError::Database("Connection closed".to_string()))
        }
    }
}

impl Drop for TransactionContext {
    fn drop(&mut self) {
        if !self.committed {
            if let Some(mut conn) = self.conn.take() {
                tokio::spawn(async move {
                    let _ = conn.rollback().await;
                });
            }
        }
    }
}
