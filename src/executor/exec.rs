use crate::Result;
use crate::error::DbError;
use crate::tpl::engine;
use crate::udbc::connection::Connection;
use crate::udbc::driver::Driver;
use crate::udbc::value::{FromValue, ToValue, Value};
use log::debug;
use std::collections::HashMap;
use std::time::Instant;

/// Executes a SQL statement (INSERT, UPDATE, DELETE) on the given connection.
pub async fn execute_conn<T: ToValue>(
    conn: &mut dyn Connection,
    driver: &dyn Driver,
    template_name: &str,
    sql: &str,
    args: &T,
) -> Result<u64> {
    let start = Instant::now();
    let (rendered_sql, params) = engine::render_template(template_name, sql, args, driver)?;
    let result = conn.execute(&rendered_sql, &params).await;
    let elapsed = start.elapsed().as_millis();

    match &result {
        Ok(affected) => debug!(
            "Execute: sql=\n{}, params={:?}, elapsed={}ms, affected={}",
            &rendered_sql, &params, elapsed, affected
        ),
        Err(e) => debug!(
            "Execute: sql=\n{}, params={:?}, elapsed={}ms, error={:?}",
            &rendered_sql, &params, elapsed, e
        ),
    }

    result
}

/// Executes a SQL query on the given connection and returns raw rows.
pub async fn query_conn<T: ToValue>(
    conn: &mut dyn Connection,
    driver: &dyn Driver,
    template_name: &str,
    sql: &str,
    args: &T,
) -> Result<Vec<HashMap<String, Value>>> {
    let start = Instant::now();
    let (rendered_sql, params) = engine::render_template(template_name, sql, args, driver)?;
    let result: Result<Vec<HashMap<String, Value>>> = conn.query(&rendered_sql, &params).await;
    let elapsed = start.elapsed().as_millis();

    match &result {
        Ok(rows) => debug!(
            "Query: sql=\n{}, params={:?}, elapsed={}ms, rows={}",
            &rendered_sql,
            &params,
            elapsed,
            rows.len()
        ),
        Err(e) => debug!(
            "Query: sql=\n{}, params={:?}, elapsed={}ms, error={:?}",
            &rendered_sql, &params, elapsed, e
        ),
    }

    result
}

/// Maps raw database rows to the target type `R`.
pub fn map_rows<R>(rows: Vec<HashMap<String, Value>>) -> Result<Vec<R>>
where
    R: FromValue,
{
    rows.into_iter()
        .map(|r| {
            R::from_value(Value::Map(r))
                .map_err(|e| DbError::SerializationError(format!("Row mapping failed: {:?}", e)))
        })
        .collect()
}
