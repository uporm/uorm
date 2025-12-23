use thiserror::Error;

/// Represents all possible errors that can occur within the `uorm` crate.
#[derive(Error, Debug)]
pub enum DbError {
    /// A generic error with a custom message.
    #[error("{0}")]
    General(String),
    
    /// An error originating from an underlying database driver.
    #[error("Driver error: {0}")]
    Driver(#[source] Box<dyn std::error::Error + Send + Sync>),
    
    /// An error related to database connection management (e.g., failed to acquire from pool).
    #[error("Connection error: {0}")]
    Connection(String),
    
    /// An error that occurred during the execution of a SQL query.
    #[error("Query error: {0}")]
    Query(String),
    
    /// An error related to value conversion or serialization/deserialization.
    #[error("Value error: {0}")]
    Value(String),
    
    /// Error indicating that a requested feature or method is not yet implemented.
    #[error("Not implemented")]
    NotImplemented,
    
    /// Error indicating that the provided database type is not supported by the current configuration.
    #[error("Unsupported database type: {0}")]
    UnsupportedDatabaseType(String),
    
    /// Error indicating that the database connection URL is malformed or invalid.
    #[error("Invalid database URL: {0}")]
    InvalidDatabaseUrl(String),
    
    /// A high-level database error, typically wrapping driver-specific errors.
    #[error("Database error: {0}")]
    Database(String),
}

impl serde::de::Error for DbError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        DbError::General(msg.to_string())
    }
}

impl serde::ser::Error for DbError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        DbError::General(msg.to_string())
    }
}

#[cfg(feature = "mysql")]
impl From<mysql_async::Error> for DbError {
    fn from(e: mysql_async::Error) -> Self {
        DbError::Database(e.to_string())
    }
}

#[cfg(feature = "sqlite")]
impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::Database(e.to_string())
    }
}
