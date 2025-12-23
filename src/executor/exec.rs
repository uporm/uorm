use crate::error::DbError;
use crate::tpl::engine;
use crate::udbc::connection::Connection;
use crate::udbc::deserializer::RowDeserializer;
use crate::udbc::driver::Driver;
use crate::udbc::value::Value;
use log::debug;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Instant;

/// Executes a SQL statement (INSERT, UPDATE, DELETE) on the given connection.
pub async fn execute_conn<T: Serialize>(
    conn: &mut dyn Connection,
    driver: &dyn Driver,
    sql: &str,
    args: &T,
) -> Result<u64, DbError> {
    let start = Instant::now();
    let (rendered_sql, params) = engine::render_template(sql, sql, args, driver)?;
    let result = conn.execute(&rendered_sql, &params).await;
    let elapsed = start.elapsed().as_millis();
    
    match &result {
        Ok(affected) => debug!("Execute: sql={}, params={:?}, elapsed={}ms, affected={}", rendered_sql, params, elapsed, affected),
        Err(e) => debug!("Execute: sql={}, params={:?}, elapsed={}ms, error={:?}", rendered_sql, params, elapsed, e),
    }

    result
}

/// Executes a SQL query on the given connection and returns raw rows.
pub async fn query_conn<T: Serialize>(
    conn: &mut dyn Connection,
    driver: &dyn Driver,
    sql: &str,
    args: &T,
) -> Result<Vec<HashMap<String, Value>>, DbError> {
    let start = Instant::now();
    let (rendered_sql, params) = engine::render_template(sql, sql, args, driver)?;
    let result = conn.query(&rendered_sql, &params).await;
    let elapsed = start.elapsed().as_millis();

    match &result {
        Ok(rows) => debug!("Query: sql={}, params={:?}, elapsed={}ms, rows={}", rendered_sql, params, elapsed, rows.len()),
        Err(e) => debug!("Query: sql={}, params={:?}, elapsed={}ms, error={:?}", rendered_sql, params, elapsed, e),
    }

    result
}

/// Maps raw database rows to the target type `R`.
pub fn map_rows<R>(rows: Vec<HashMap<String, Value>>) -> Result<Vec<R>, DbError>
where
    R: serde::de::DeserializeOwned,
{
    rows.into_iter()
        .map(|r| {
            R::deserialize(RowDeserializer::new(&r))
                .map_err(|e| DbError::General(format!("Row mapping failed: {}", e)))
        })
        .collect()
}
