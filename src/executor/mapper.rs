use crate::Result;
use crate::error::DbError;
use crate::executor::session::Session;
use crate::mapper_loader::{SqlStatement, StatementType, find_statement};
use crate::udbc::driver::Driver;
use crate::udbc::value::{FromValue, ToValue, Value};
use std::sync::Arc;

/// 封装连接池和 SQL 模板执行的 Mapper 客户端。
///
/// 作为 `Session` 的高层抽象，负责 SQL ID 查找与按语句类型做结果映射。
pub struct Mapper {
    pub pool: Arc<dyn Driver>,
}

impl Mapper {
    pub fn new(pool: Arc<dyn Driver>) -> Self {
        Self { pool }
    }

    /// 为该 mapper 创建一个临时 session。
    /// 注意：创建 session 的成本很低（仅 Arc 克隆）。
    fn session(&self) -> Session {
        Session::new(self.pool.clone())
    }

    fn get_statement(&self, sql_id: &str) -> Result<Arc<SqlStatement>> {
        find_statement(sql_id, self.pool.r#type())
            .ok_or_else(|| DbError::TemplateEngineError(format!("SQL ID not found: {}", sql_id)))
    }

    async fn execute_insert_with_return_key<T: ToValue>(
        &self,
        session: &Session,
        sql_id: &str,
        sql: &str,
        args: &T,
    ) -> Result<Value> {
        if session.is_transaction_active() {
            let _ = session.execute_named(sql_id, sql, args).await?;
            let id = session.last_insert_id().await?;
            return Ok(Value::U64(id));
        }

        crate::executor::session::with_tx_context(|| async {
            session.begin().await?;
            let result = async {
                let _ = session.execute_named(sql_id, sql, args).await?;
                session.last_insert_id().await
            }
            .await;

            match result {
                Ok(id) => {
                    session.commit().await?;
                    Ok(Value::U64(id))
                }
                Err(e) => {
                    session.rollback().await?;
                    Err(e)
                }
            }
        })
        .await
    }

    /// 按 ID 执行映射后的 SQL 语句。
    ///
    /// # 泛型参数
    /// * `R`：返回类型。必须可从数据库值转换（同时支持 Serde 与 FromRow）。
    ///   - 对 `Select`，`R` 通常为 `Vec<T>`。
    ///   - 对 `Insert`/`Update`/`Delete`，`R` 通常为 `u64`（影响行数）或 `i64`。
    /// * `T`：参数类型。必须可序列化（传入模板引擎）。
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
                            Err(_) => {
                                let map_value = Value::Map(row.clone());
                                match R::from_value(map_value) {
                                    Ok(v) => return Ok(v),
                                    Err(map_err) => {
                                        if row.len() == 1 {
                                            let (_, only_val) = row.into_iter().next().unwrap();
                                            match R::from_value(only_val) {
                                                Ok(v) => return Ok(v),
                                                Err(_) => {
                                                     // 若单值映射也失败，返回 map 映射错误
                                                     // 因为用户更可能希望映射到结构体
                                                     return Err(map_err);
                                                }
                                            }
                                        }
                                        return Err(map_err);
                                    }
                                }
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
                    self.execute_insert_with_return_key(&session, sql_id, sql, args)
                        .await?
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
