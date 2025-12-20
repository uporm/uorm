use crate::error::DbError;
use crate::executor::session::Session;
use crate::mapper_loader::find_mapper;
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

    fn get_sql_mapper(
        &self,
        sql_id: &str,
    ) -> Result<std::sync::Arc<crate::mapper_loader::SqlMapper>, DbError> {
        find_mapper(sql_id, self.pool.r#type())
            .ok_or_else(|| DbError::Query(format!("SQL ID not found: {}", sql_id)))
    }

    pub async fn get<R, T>(&self, sql_id: &str, args: &T) -> Result<R, DbError>
    where
        T: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let mapper = self.get_sql_mapper(sql_id)?;
        let sql = mapper
            .as_ref()
            .content
            .as_ref()
            .ok_or_else(|| DbError::Query(format!("SQL content empty for {}", sql_id)))?;
        let mut rows: Vec<R> = self.session().query(sql, args).await?;
        if rows.len() > 1 {
            return Err(DbError::Query("Expected 1 row, got multiple".into()));
        }
        rows.pop().ok_or(DbError::Query("No row found".into()))
    }

    pub async fn list<R, T>(&self, sql_id: &str, args: &T) -> Result<Vec<R>, DbError>
    where
        T: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let mapper = self.get_sql_mapper(sql_id)?;
        let sql = mapper
            .as_ref()
            .content
            .as_deref()
            .ok_or_else(|| DbError::Query(format!("SQL content empty for {}", sql_id)))?;
        self.session().query(sql, args).await
    }

    pub async fn insert<R, T>(&self, sql_id: &str, args: &T) -> Result<R, DbError>
    where
        T: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let mapper = self.get_sql_mapper(sql_id)?;
        let sql = mapper
            .as_ref()
            .content
            .as_deref()
            .ok_or_else(|| DbError::Query(format!("SQL content empty for {}", sql_id)))?;
        let session = self.session();

        let affected = session.execute(sql, args).await?;

        if mapper.use_generated_keys {
            let id = session.last_insert_id().await?;
            let v = Value::I64(id as i64);
            R::deserialize(ValueDeserializer { value: &v })
        } else {
            // Try to return affected rows as R
            let v = Value::I64(affected as i64);
            R::deserialize(ValueDeserializer { value: &v })
        }
    }

    // pub async fn batch_insert<R, T>(&self, sql_id: &str, args: &[T]) -> Result<Vec<R>, DbError>
    // where
    //     T: serde::Serialize,
    //     R: serde::de::DeserializeOwned,
    // {
    //     let mapper = self.get_sql_mapper(sql_id)?;
    //     let sql = mapper
    //         .as_ref()
    //         .content
    //         .as_ref()
    //         .ok_or_else(|| DbError::Query(format!("SQL content empty for {}", sql_id)))?;
    //     let session = self.session();
    //
    //     let mut results = Vec::with_capacity(args.len());
    //
    //     for arg in args {
    //         let affected = session.execute(sql, arg).await?;
    //         let val = if mapper.use_generated_keys {
    //             let id = session.last_insert_id().await?;
    //             Value::I64(id as i64)
    //         } else {
    //             Value::I64(affected as i64)
    //         };
    //
    //         let r = R::deserialize(ValueDeserializer { value: &val })?;
    //         results.push(r);
    //     }
    //     Ok(results)
    // }

    pub async fn update<T>(&self, sql_id: &str, args: &T) -> Result<u64, DbError>
    where
        T: serde::Serialize,
    {
        let mapper = self.get_sql_mapper(sql_id)?;
        let sql = mapper
            .as_ref()
            .content
            .as_deref()
            .ok_or_else(|| DbError::Query(format!("SQL content empty for {}", sql_id)))?;
        self.session().execute(sql, args).await
    }

    pub async fn delete<T>(&self, sql_id: &str, args: &T) -> Result<u64, DbError>
    where
        T: serde::Serialize,
    {
        let mapper = self.get_sql_mapper(sql_id)?;
        let sql = mapper
            .content
            .as_ref()
            .ok_or_else(|| DbError::Query(format!("SQL content empty for {}", sql_id)))?;
        self.session().execute(sql, args).await
    }
}
