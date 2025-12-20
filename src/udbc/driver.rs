use crate::error::DbError;
use crate::udbc::connection::Connection;
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait Driver: Send + Sync {
    fn name(&self) -> &str;

    fn r#type(&self) -> &str;

    fn placeholder(&self, param_seq: usize, param_name: &str) -> String;

    async fn connection(&self) -> Result<Arc<dyn Connection>, DbError>;
    async fn close(&self) -> Result<(), DbError>;
}
