use crate::error::DbError;
use crate::tpl::engine;
use crate::transaction::TransactionContext;
use crate::udbc::deserializer::RowDeserializer;
use crate::udbc::driver::Driver;
use crate::udbc::value::Value;
use log::debug;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

type TransactionContextMap = HashMap<String, Arc<Mutex<TransactionContext>>>;

thread_local! {
    static TX_CONTEXT: RefCell<TransactionContextMap> = RefCell::new(HashMap::new());
}

/// Database session wrapper managing connection pools and transaction state.
///
/// Provides a unified interface for executing queries whether inside a transaction or not.
pub struct Session {
    pool: Arc<dyn Driver>,
}

impl Session {
    pub fn new(pool: Arc<dyn Driver>) -> Self {
        Self { pool }
    }

    /// Begins a new transaction for the current database connection.
    ///
    /// The transaction state is stored in a thread-local map (`TX_CONTEXT`) using the driver's name as the key.
    /// This ensures that nested or subsequent calls within the same thread can access the active transaction.
    /// 
    /// # Errors
    /// Returns `DbError::General` if a transaction has already been started for this driver in the current thread.
    pub async fn begin(&self) -> Result<(), DbError> {
        let key = self.pool.name().to_string();
        let existed = TX_CONTEXT.with(|tx| tx.borrow().contains_key(&key));
        if existed {
            return Err(DbError::General(format!(
                "Transaction already started for '{}'",
                key
            )));
        }

        let ctx = TransactionContext::begin(self.pool.clone()).await?;
        TX_CONTEXT.with(|tx| {
            tx.borrow_mut().insert(key, Arc::new(Mutex::new(ctx)));
        });
        Ok(())
    }

    /// Commits the active transaction for the current database connection.
    ///
    /// If no transaction is active, this method does nothing and returns `Ok(())`.
    /// Upon completion, the transaction context is removed from the thread-local storage.
    pub async fn commit(&self) -> Result<(), DbError> {
        let key = self.pool.name().to_string();
        let tx = TX_CONTEXT.with(|map| map.borrow().get(&key).cloned());
        let Some(tx) = tx else {
            return Ok(());
        };

        {
            let mut ctx = tx.lock().await;
            ctx.commit().await?;
        }

        // Clean up the transaction context from thread-local storage.
        TX_CONTEXT.with(|map| {
            map.borrow_mut().remove(&key);
        });
        Ok(())
    }

    /// Rolls back the active transaction for the current database connection.
    ///
    /// If no transaction is active, this method does nothing and returns `Ok(())`.
    /// Upon completion, the transaction context is removed from the thread-local storage.
    pub async fn rollback(&self) -> Result<(), DbError> {
        let key = self.pool.name().to_string();
        let tx = TX_CONTEXT.with(|map| map.borrow().get(&key).cloned());
        let Some(tx) = tx else {
            return Ok(());
        };

        {
            let mut ctx = tx.lock().await;
            ctx.rollback().await?;
        }

        // Clean up the transaction context from thread-local storage.
        TX_CONTEXT.with(|map| {
            map.borrow_mut().remove(&key);
        });
        Ok(())
    }

    /// Executes a SQL statement (e.g., INSERT, UPDATE, DELETE) that modifies data.
    ///
    /// # Arguments
    /// * `sql` - The SQL template to execute.
    /// * `args` - Parameters to be bound to the SQL template.
    ///
    /// # Returns
    /// The number of rows affected by the statement.
    ///
    /// This method automatically detects if it's running within an active transaction. 
    /// If so, it delegates execution to the transaction context. Otherwise, it renders 
    /// the template and executes it directly on a connection from the pool.
    pub async fn execute<T>(&self, sql: &str, args: &T) -> Result<u64, DbError>
    where
        T: serde::Serialize,
    {
        let start = Instant::now();
        let result = self.execute_impl(sql, args).await;
        self.log_cmd("execute", sql, start.elapsed().as_millis(), &result);
        result
    }

    async fn execute_impl<T>(&self, sql: &str, args: &T) -> Result<u64, DbError>
    where
        T: serde::Serialize,
    {
        let key = self.pool.name().to_string();
        // Check if there's an active transaction for this driver.
        if let Some(tx) = TX_CONTEXT.with(|map| map.borrow().get(&key).cloned()) {
            let mut ctx = tx.lock().await;
            return ctx.execute(sql, args).await;
        }

        // No active transaction, render template and execute on a new connection.
        let (rendered_sql, params) = engine::render_template(sql, sql, args, self.pool.as_ref())?;

        debug!("Execute (Conn): sql={}, params={:?}", rendered_sql, params);

        let mut conn = self.pool.acquire().await?;
        conn.execute(&rendered_sql, &params).await
    }

    /// Executes a SQL query and maps the resulting rows to a collection of type `R`.
    ///
    /// # Arguments
    /// * `sql` - The SQL template to execute.
    /// * `args` - Parameters to be bound to the SQL template.
    ///
    /// # Returns
    /// A `Vec<R>` containing the deserialized results.
    pub async fn query<R, T>(&self, sql: &str, args: &T) -> Result<Vec<R>, DbError>
    where
        T: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let rows = self.query_raw(sql, args).await?;
        Self::map_rows(rows)
    }

    /// Executes a SQL query and returns the results as a list of raw HashMaps.
    ///
    /// Each HashMap represents a row, mapping column names to their values.
    pub async fn query_raw<T>(
        &self,
        sql: &str,
        args: &T,
    ) -> Result<Vec<HashMap<String, Value>>, DbError>
    where
        T: serde::Serialize,
    {
        let start = Instant::now();
        let result = self.query_raw_impl(sql, args).await;

        // Log query performance and row count.
        let elapsed = start.elapsed().as_millis();
        match &result {
            Ok(rows) => debug!(
                "query: sql={}, elapsed={}ms, rows={}",
                sql,
                elapsed,
                rows.len()
            ),
            Err(e) => debug!("query: sql={}, elapsed={}ms, error={:?}", sql, elapsed, e),
        }

        result
    }

    async fn query_raw_impl<T>(
        &self,
        sql: &str,
        args: &T,
    ) -> Result<Vec<HashMap<String, Value>>, DbError>
    where
        T: serde::Serialize,
    {
        let key = self.pool.name().to_string();
        if let Some(tx) = TX_CONTEXT.with(|map| map.borrow().get(&key).cloned()) {
            let mut ctx = tx.lock().await;
            return ctx.query(sql, args).await;
        }

        let (rendered_sql, params) = engine::render_template(sql, sql, args, self.pool.as_ref())?;

        debug!("Query (Conn): sql={}, params={:?}", rendered_sql, params);

        let mut conn = self.pool.acquire().await?;
        conn.query(&rendered_sql, &params).await
    }

    /// Maps raw database rows to the target type `R`.
    ///
    /// Performance Note: Iterates and deserializes each row individually.
    /// This is generally efficient but depends on the complexity of `R`.
    fn map_rows<R>(rows: Vec<HashMap<String, Value>>) -> Result<Vec<R>, DbError>
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

    /// Retrieves the ID of the last inserted row.
    pub async fn last_insert_id(&self) -> Result<u64, DbError> {
        let key = self.pool.name().to_string();
        if let Some(tx) = TX_CONTEXT.with(|map| map.borrow().get(&key).cloned()) {
            let mut ctx = tx.lock().await;
            return ctx.last_insert_id().await;
        }

        let mut conn = self.pool.acquire().await?;
        conn.last_insert_id().await
    }

    /// unified logging helper for execution results
    fn log_cmd<D: std::fmt::Debug>(
        &self,
        op: &str,
        sql: &str,
        elapsed: u128,
        result: &Result<D, DbError>,
    ) {
        match result {
            Ok(val) => debug!(
                "{}: sql={}, elapsed={}ms, affected/result={:?}",
                op, sql, elapsed, val
            ),
            Err(e) => debug!("{}: sql={}, elapsed={}ms, error={:?}", op, sql, elapsed, e),
        }
    }
}
