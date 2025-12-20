use crate::error::DbError;
use crate::udbc::value::Value;
use async_trait::async_trait;
use std::collections::HashMap;

#[async_trait]
pub trait Connection: Send + Sync {
    async fn query(
        &self,
        sql: &str,
        args: &[(String, Value)],
    ) -> Result<Vec<HashMap<String, Value>>, DbError>;

    async fn execute(&self, sql: &str, args: &[(String, Value)]) -> Result<u64, DbError>;

    async fn last_insert_id(&self) -> Result<u64, DbError>;

    // ---------- transaction ----------
    async fn begin(&self) -> Result<(), DbError>;
    async fn commit(&self) -> Result<(), DbError>;
    async fn rollback(&self) -> Result<(), DbError>;
}
