use crate::error::DbError;
use crate::tpl::engine;
use crate::transaction::TransactionContext;
use crate::udbc::deserializer::RowDeserializer;
use crate::udbc::driver::Driver;
use crate::udbc::value::Value;
use log::debug;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::task_local;

task_local! {
    /// 当前任务的事务上下文
     static TX_CONTEXT: Arc<tokio::sync::Mutex<TransactionContext>>;
}

/// 数据库客户端，封装了连接池操作
pub struct Session {
    pool: Arc<dyn Driver>,
}

impl Session {
    pub fn new(pool: Arc<dyn Driver>) -> Self {
        Self { pool }
    }

    pub async fn begin(&self) -> Result<TransactionContext, DbError> {
        TransactionContext::begin(self.pool.clone()).await
    }

    pub async fn execute<T>(&self, sql: &str, args: &T) -> Result<u64, DbError>
    where
        T: serde::Serialize,
    {
        if let Ok(ctx) = TX_CONTEXT.try_with(|tx| tx.clone()) {
            let start = Instant::now();
            let result = ctx.lock().await.execute(sql, args).await;
            let elapsed_ms = start.elapsed().as_millis();
            let affected = result.as_ref().ok().copied();
            let err = result.as_ref().err().map(|e| e.to_string());
            debug!(
                "execute: sql={}, elapsed_ms={}, affected={:?}, error={:?}",
                sql, elapsed_ms, affected, err
            );
            result
        } else {
            let (rendered_sql, params) =
                engine::render_template(sql, sql, args, self.pool.as_ref());
            let conn = self.pool.connection().await?;
            let start = Instant::now();
            let result = conn.execute(&rendered_sql, &params).await;
            let elapsed_ms = start.elapsed().as_millis();
            let affected = result.as_ref().ok().copied();
            let err = result.as_ref().err().map(|e| e.to_string());
            debug!(
                "Preparing query: sql={}, params={:?}, elapsed_ms={}, affected={:?}, error={:?}",
                rendered_sql, params, elapsed_ms, affected, err
            );
            result
        }
    }

    pub async fn query<R, T>(&self, sql: &str, args: &T) -> Result<Vec<R>, DbError>
    where
        T: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let rows = if let Ok(ctx) = TX_CONTEXT.try_with(|tx| tx.clone()) {
            let start = Instant::now();
            let rows = ctx.lock().await.query(sql, args).await?;
            let elapsed_ms = start.elapsed().as_millis();
            debug!(
                "query: sql={}, elapsed_ms={}, rows={}",
                sql,
                elapsed_ms,
                rows.len()
            );
            rows
        } else {
            let (rendered_sql, params) =
                engine::render_template(sql, sql, args, self.pool.as_ref());
            let conn = self.pool.connection().await?;
            let start = Instant::now();
            let rows = conn.query(&rendered_sql, &params).await?;
            let elapsed_ms = start.elapsed().as_millis();
            debug!(
                "Preparing query: sql={}, params={:?}, elapsed_ms={}, rows={}",
                rendered_sql,
                params,
                elapsed_ms,
                rows.len()
            );
            rows
        };
        Self::map_rows(rows)
    }

    /// 将行数据映射为目标类型
    fn map_rows<R>(rows: Vec<HashMap<String, Value>>) -> Result<Vec<R>, DbError>
    where
        R: serde::de::DeserializeOwned,
    {
        rows.into_iter()
            .map(|r| {
                R::deserialize(RowDeserializer::new(&r))
                    .map_err(|e| DbError::General(e.to_string()))
            })
            .collect()
    }

    pub async fn last_insert_id(&self) -> Result<u64, DbError> {
        if let Ok(ctx) = TX_CONTEXT.try_with(|tx| tx.clone()) {
            ctx.lock().await.last_insert_id().await
        } else {
            let conn = self.pool.connection().await?;
            conn.last_insert_id().await
        }
    }
}
