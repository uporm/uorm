use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Database Error: {0}")]
    DbError(String),
    #[error("Invalid Database Url: {0}")]
    DbUrlError(String),
    #[error("Serialization Error: {0}")]
    SerializationError(String),
    #[error("Query Build Error: {0}")]
    QueryBuildError(String),
    #[error("Data Conversion Error: {0}")]
    DataConversionError(String),
    #[error("Mapper Load Error: {0}")]
    MapperLoadError(String),
    #[error("Driver Error: {0}")]
    DriverError(String),
    #[error("Template Engine Error: {0}")]
    TemplateEngineError(String),
    #[error("SQL Execution Error: {0}")]
    SqlExecutionError(String),
}

// Aliases for compatibility
pub type Error = DbError;
pub type SerdeError = DbError;


impl serde::ser::Error for DbError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        DbError::SerializationError(msg.to_string())
    }
}

impl serde::de::Error for DbError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        DbError::SerializationError(msg.to_string())
    }
}
