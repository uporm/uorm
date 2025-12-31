use crate::Result;
use crate::error::DbError;
use crate::executor::session::Session;
use crate::mapper_loader::{SqlStatement, StatementType, find_statement};
use crate::udbc::driver::Driver;
use crate::udbc::value::{FromValue, ToValue, Value};
use std::sync::Arc;

/// Mapper client encapsulating connection pool and SQL template execution.
///
/// Acts as a higher-level abstraction over `Session`, handling SQL ID lookup
/// and result mapping based on statement type.
pub struct Mapper {
    pub pool: Arc<dyn Driver>,
}

impl Mapper {
    pub fn new(pool: Arc<dyn Driver>) -> Self {
        Self { pool }
    }

    /// Creates a new ephemeral session for this mapper.
    /// Note: Sessions are cheap to create (Arc clone).
    fn session(&self) -> Session {
        Session::new(self.pool.clone())
    }

    fn get_statement(&self, sql_id: &str) -> Result<Arc<SqlStatement>> {
        find_statement(sql_id, self.pool.r#type())
            .ok_or_else(|| DbError::TemplateEngineError(format!("SQL ID not found: {}", sql_id)))
    }

    /// Executes a mapped SQL statement by ID.
    ///
    /// # Generic Parameters
    /// * `R`: Return type. Must be convertible from a database value (supports both Serde and FromRow).
    ///   - For `Select`, `R` is typically `Vec<T>`.
    ///   - For `Insert`/`Update`/`Delete`, `R` is typically `u64` (affected rows) or `i64`.
    /// * `T`: Argument type. Must be serializable (passed to the template engine).
    pub async fn execute<R, T>(&self, sql_id: &str, args: &T) -> Result<R>
    where
        T: ToValue,
        R: FromValue,
    {
        let stmt = self.get_statement(sql_id)?;
        let sql = stmt.as_ref().content.as_deref().ok_or_else(|| {
            DbError::TemplateEngineError(format!("SQL content empty for {}", sql_id))
        })?;

        match stmt.r#type {
            StatementType::Select => {
                let rows: Vec<std::collections::HashMap<String, Value>> =
                    self.session().query_raw_named(sql_id, sql, args).await?;

                // Performance Note:
                // We convert Vec<HashMap> -> Value::List(Vec<Value::Map>) -> R.
                // This intermediate step allocates. Optimizing this would require
                // a custom Deserializer that accepts Vec<HashMap> directly,
                // or changing the Session API to return R directly.
                // Given the current architecture, this is the safe approach.
                let value = Value::List(rows.into_iter().map(Value::Map).collect());
                Ok(R::from_value(value)?)
            }
            StatementType::Insert => {
                let session = self.session();
                let affected = session.execute_named(sql_id, sql, args).await?;

                let val = if stmt.use_generated_keys {
                    session.last_insert_id().await? as i64
                } else {
                    affected as i64
                };

                Ok(R::from_value(Value::I64(val))?)
            }
            StatementType::Update | StatementType::Delete | StatementType::Sql => {
                let affected = self.session().execute_named(sql_id, sql, args).await?;
                Ok(R::from_value(Value::I64(affected as i64))?)
            }
        }
    }
}
