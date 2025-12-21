use crate::error::DbError;
use crate::udbc::value::Value;
use async_trait::async_trait;
use std::collections::HashMap;

/// An abstract database connection trait that defines the basic operations
/// for interacting with a database.
#[async_trait]
pub trait Connection: Send {
    /// Execute a query statement and return the result set.
    ///
    /// # Arguments
    /// * `sql` - The SQL query string to execute
    /// * `args` - Parameters to bind to the SQL query
    ///
    /// # Returns
    /// A vector of hash maps where each hash map represents a row with column names as keys
    async fn query(
        &mut self,
        sql: &str,
        args: &[(String, Value)],
    ) -> Result<Vec<HashMap<String, Value>>, DbError>;

    /// Execute a non-query statement (INSERT, UPDATE, DELETE) and return the number of affected rows.
    ///
    /// # Arguments
    /// * `sql` - The SQL statement to execute
    /// * `args` - Parameters to bind to the SQL statement
    ///
    /// # Returns
    /// The number of affected rows
    async fn execute(&mut self, sql: &str, args: &[(String, Value)]) -> Result<u64, DbError>;

    /// Get the ID of the last inserted row.
    ///
    /// # Returns
    /// The ID of the last inserted row
    async fn last_insert_id(&mut self) -> Result<u64, DbError>;

    // ---------- transaction ----------
    /// Begin a transaction
    async fn begin(&mut self) -> Result<(), DbError>;
    /// Commit the current transaction
    async fn commit(&mut self) -> Result<(), DbError>;
    /// Rollback the current transaction
    async fn rollback(&mut self) -> Result<(), DbError>;
}
