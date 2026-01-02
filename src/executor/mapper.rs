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

                match rows.len() {
                    0 => {
                        let list_value = Value::List(Vec::new());
                        if let Ok(v) = R::from_value(list_value) {
                            return Ok(v);
                        }
                        if let Ok(v) = R::from_value(Value::Null) {
                            return Ok(v);
                        }
                        Err(DbError::DbError(format!("No rows returned for {}", sql_id)))
                    }
                    1 => {
                        let row = rows.into_iter().next().unwrap();

                        let list_value = Value::List(vec![Value::Map(row.clone())]);
                        match R::from_value(list_value) {
                            Ok(v) => Ok(v),
                            Err(list_err) => {
                                let map_value = Value::Map(row.clone());
                                if let Ok(v) = R::from_value(map_value) {
                                    return Ok(v);
                                }

                                if row.len() == 1 {
                                    let (_, only_val) = row.into_iter().next().unwrap();
                                    match R::from_value(only_val) {
                                        Ok(v) => return Ok(v),
                                        Err(e) => return Err(e),
                                    }
                                }

                                Err(list_err)
                            }
                        }
                    }
                    _ => {
                        let value = Value::List(rows.into_iter().map(Value::Map).collect());
                        Ok(R::from_value(value)?)
                    }
                }
            }
            StatementType::Insert => {
                let session = self.session();

                let val = if stmt.return_key {
                    let is_active = session.is_transaction_active();
                    if is_active {
                        let _ = session.execute_named(sql_id, sql, args).await?;
                        let id = session.last_insert_id().await?;
                        Value::U64(id)
                    } else {
                        // Use transaction to ensure same connection for insert and last_insert_id
                        session.begin().await?;
                        let result = async {
                            let _ = session.execute_named(sql_id, sql, args).await?;
                            session.last_insert_id().await
                        }
                        .await;

                        match result {
                            Ok(id) => {
                                session.commit().await?;
                                Value::U64(id)
                            }
                            Err(e) => {
                                session.rollback().await?;
                                return Err(e);
                            }
                        }
                    }
                } else {
                    let affected = session.execute_named(sql_id, sql, args).await?;
                    Value::U64(affected)
                };

                Ok(R::from_value(val)?)
            }
            StatementType::Update | StatementType::Delete | StatementType::Sql => {
                let affected = self.session().execute_named(sql_id, sql, args).await?;
                Ok(R::from_value(Value::U64(affected))?)
            }
        }
    }
}
