use thiserror::Error;

/// Represents errors that can occur in the RDBC module.
#[derive(Error, Debug)]
pub enum DbError {
    #[error("{0}")]
    General(String),
    #[error("Driver error: {0}")]
    Driver(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Query error: {0}")]
    Query(String),
    #[error("Value error: {0}")]
    Value(String),
    #[error("Not implemented")]
    NotImplemented,
    #[error("Unsupported database type: {0}")]
    UnsupportedDatabaseType(String),
    #[error("Invalid database URL: {0}")]
    InvalidDatabaseUrl(String),
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
