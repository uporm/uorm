use crate::udbc::connection::Connection;
use crate::udbc::driver::Driver;
use std::sync::Arc;
use crate::Result;

pub(crate) struct TransactionContext {
    conn: Option<Box<dyn Connection>>,
    committed: bool,
}

impl TransactionContext {
    pub async fn begin(pool: Arc<dyn Driver>) -> Result<Self> {
        let mut conn: Box<dyn Connection> = pool.acquire().await?;
        conn.begin().await?;
        Ok(Self {
            conn: Some(conn),
            committed: false,
        })
    }

    pub async fn commit(&mut self) -> Result<()> {
        if let Some(conn) = self.conn.as_mut() {
            conn.commit().await?;
        }
        self.committed = true;
        Ok(())
    }

    pub async fn rollback(&mut self) -> Result<()> {
        let r = if let Some(conn) = self.conn.as_mut() {
            conn.rollback().await
        } else {
            Ok(())
        };
        if r.is_ok() {
            self.committed = true;
        }
        r
    }
    pub fn connection_mut(&mut self) -> Option<&mut Box<dyn Connection>> {
        self.conn.as_mut()
    }
}

impl Drop for TransactionContext {
    fn drop(&mut self) {
        if !self.committed
            && let Some(mut conn) = self.conn.take()
        {
            tokio::spawn(async move {
                let _ = conn.rollback().await;
            });
        }
    }
}
