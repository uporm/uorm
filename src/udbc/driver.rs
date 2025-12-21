use crate::error::DbError;
use crate::udbc::connection::Connection;
use async_trait::async_trait;

/// `Driver` defines a common interface for database drivers.
///
/// A driver is responsible for:
/// - Providing metadata about itself (name, type)
/// - Generating parameter placeholders for SQL queries
/// - Managing database connections
/// - Cleaning up resources when closed
#[async_trait]
pub trait Driver: Send + Sync {
    /// Returns the name of the driver.
    ///
    /// Example: "postgres", "mysql", "sqlite"
    fn name(&self) -> &str;

    /// Returns the type of the driver.
    ///
    /// This can be used to distinguish between different database categories
    /// or protocols.
    fn r#type(&self) -> &str;

    /// Generates a placeholder string for a query parameter.
    ///
    /// # Arguments
    /// * `param_seq` - The sequential index of the parameter (starting from 1)
    /// * `param_name` - The logical name of the parameter
    ///
    /// # Returns
    /// A database-specific placeholder string.
    ///
    /// Example outputs:
    /// - PostgreSQL: `$1`
    /// - MySQL / SQLite: `?`
    /// - Named parameters: `:param_name`
    fn placeholder(&self, param_seq: usize, param_name: &str) -> String;

    /// Creates and returns a new database connection.
    ///
    /// # Returns
    /// - `Ok(Box<dyn Connection>)` if the connection is successfully established
    /// - `Err(DbError)` if connection creation fails
    async fn acquire(&self) -> Result<Box<dyn Connection>, DbError>;

    /// Closes the driver and releases any associated resources.
    ///
    /// This should be called when the driver is no longer needed.
    ///
    /// # Returns
    /// - `Ok(())` if cleanup succeeds
    /// - `Err(DbError)` if an error occurs during cleanup
    async fn close(&self) -> Result<(), DbError>;
}
