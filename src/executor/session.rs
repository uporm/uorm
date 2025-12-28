use crate::error::DbError;
use crate::Result;
use crate::executor::exec::{execute_conn, map_rows, query_conn};
use crate::executor::transaction::TransactionContext;
use crate::udbc::connection::Connection;
use crate::udbc::driver::Driver;
use crate::udbc::value::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
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
    /// Returns `Error` if a transaction has already been started for this driver in the current thread.
    pub async fn begin(&self) -> Result<()> {
        let key = self.pool.name().to_string();
        let existed = TX_CONTEXT.with(|tx| tx.borrow().contains_key(&key));
        if existed {
            return Err(DbError::DbError(format!(
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
    pub async fn commit(&self) -> Result<()> {
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
    pub async fn rollback(&self) -> Result<()> {
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
    pub async fn execute<T>(&self, sql: &str, args: &T) -> Result<u64>
    where
        T: serde::Serialize,
    {
        let key = self.pool.name().to_string();
        // Check if there's an active transaction for this driver.
        if let Some(tx) = TX_CONTEXT.with(|map| map.borrow().get(&key).cloned()) {
            let mut ctx = tx.lock().await;
            if let Some(conn) = ctx.connection_mut() {
                return execute_conn(conn.as_mut(), self.pool.as_ref(), sql, args).await;
            } else {
                return Err(DbError::DbError(
                    "Transaction connection closed".to_string(),
                ));
            }
        }

        // No active transaction, render template and execute on a new connection.
        let mut conn: Box<dyn Connection> = self.pool.acquire().await?;
        execute_conn(conn.as_mut(), self.pool.as_ref(), sql, args).await
    }

    /// Executes a SQL query and maps the resulting rows to a collection of type `R`.
    ///
    /// # Arguments
    /// * `sql` - The SQL template to execute.
    /// * `args` - Parameters to be bound to the SQL template.
    ///
    /// # Returns
    /// A `Vec<R>` containing the deserialized results.
    pub async fn query<R, T>(&self, sql: &str, args: &T) -> Result<Vec<R>>
    where
        T: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let rows = self.query_raw(sql, args).await?;
        map_rows(rows)
    }

    /// Executes a SQL query and returns the results as a list of raw HashMaps.
    ///
    /// Each HashMap represents a row, mapping column names to their values.
    pub async fn query_raw<T>(
        &self,
        sql: &str,
        args: &T,
    ) -> Result<Vec<HashMap<String, Value>>>
    where
        T: serde::Serialize,
    {
        let key = self.pool.name().to_string();
        if let Some(tx) = TX_CONTEXT.with(|map| map.borrow().get(&key).cloned()) {
            let mut ctx = tx.lock().await;
            if let Some(conn) = ctx.connection_mut() {
                return query_conn(conn.as_mut(), self.pool.as_ref(), sql, args).await;
            } else {
                return Err(DbError::DbError(
                    "Transaction connection closed".to_string(),
                ));
            }
        }

        let mut conn: Box<dyn Connection> = self.pool.acquire().await?;
        query_conn(conn.as_mut(), self.pool.as_ref(), sql, args).await
    }

    /// Retrieves the ID of the last inserted row.
    pub async fn last_insert_id(&self) -> Result<u64> {
        let key = self.pool.name().to_string();
        if let Some(tx) = TX_CONTEXT.with(|map| map.borrow().get(&key).cloned()) {
            let mut ctx = tx.lock().await;
            if let Some(conn) = ctx.connection_mut() {
                return conn.last_insert_id().await;
            } else {
                return Err(DbError::DbError(
                    "Transaction connection closed".to_string(),
                ));
            }
        }

        let mut conn: Box<dyn Connection> = self.pool.acquire().await?;
        conn.last_insert_id().await
    }
}
