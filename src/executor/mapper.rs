use crate::error::DbError;
use crate::executor::session::Session;
use crate::mapper_loader::{find_statement, SqlStatement, StatementType};
use crate::udbc::deserializer::ValueDeserializer;
use crate::udbc::driver::Driver;
use crate::udbc::value::Value;
use std::sync::Arc;

/// 映射器客户端，封装了连接池与模板调用
pub struct Mapper {
    pool: Arc<dyn Driver>,
}

impl Mapper {
    pub fn new(pool: Arc<dyn Driver>) -> Self {
        Self { pool }
    }

    fn session(&self) -> Session {
        Session::new(self.pool.clone())
    }

    fn get_statement(&self, sql_id: &str) -> Result<Arc<SqlStatement>, DbError> {
        find_statement(sql_id, self.pool.r#type())
            .ok_or_else(|| DbError::Query(format!("SQL ID not found: {}", sql_id)))
    }

    pub async fn execute<R, T>(&self, sql_id: &str, args: &T) -> Result<R, DbError>
    where
        T: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let stmt = self.get_statement(sql_id)?;
        let sql = stmt
            .as_ref()
            .content
            .as_deref()
            .ok_or_else(|| DbError::Query(format!("SQL content empty for {}", sql_id)))?;

        match stmt.r#type {
            StatementType::Select => {
                let rows: Vec<Value> = self.session().query(sql, args).await?;
                let value = Value::List(rows);
                R::deserialize(ValueDeserializer { value: &value })
            }
            StatementType::Insert => {
                let session = self.session();
                let affected = session.execute(sql, args).await?;
                if stmt.use_generated_keys {
                    let id = session.last_insert_id().await?;
                    let v = Value::I64(id as i64);
                    R::deserialize(ValueDeserializer { value: &v })
                } else {
                    let v = Value::I64(affected as i64);
                    R::deserialize(ValueDeserializer { value: &v })
                }
            }
            StatementType::Update | StatementType::Delete | StatementType::Sql => {
                let affected = self.session().execute(sql, args).await?;
                let v = Value::I64(affected as i64);
                R::deserialize(ValueDeserializer { value: &v })
            }
        }
    }
}
