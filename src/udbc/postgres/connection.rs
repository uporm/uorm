use async_trait::async_trait;
use deadpool_postgres::Object;
use tokio_postgres::types::ToSql;

use crate::Result;
use crate::error::DbError;
use crate::udbc::connection::Connection;
use crate::udbc::postgres::value_codec::{from_pg_row, to_pg_param};
use crate::udbc::value::Value;

pub struct PostgresConnection {
    client: Option<Object>,
}

impl PostgresConnection {
    pub fn new(client: Object) -> Self {
        Self {
            client: Some(client),
        }
    }

    fn client_mut(&mut self) -> Result<&mut Object> {
        self.client
            .as_mut()
            .ok_or_else(|| DbError::DbError("Connection closed".to_string()))
    }
}

#[async_trait]
impl Connection for PostgresConnection {
    async fn query(
        &mut self,
        sql: &str,
        args: &[(String, Value)],
    ) -> Result<Vec<std::collections::HashMap<String, Value>>> {
        let params: Vec<Box<dyn ToSql + Sync + Send>> =
            args.iter().map(|(_, v)| to_pg_param(v)).collect();
        let param_refs: Vec<&(dyn ToSql + Sync)> =
            params.iter().map(|v| &**v as &(dyn ToSql + Sync)).collect();

        let rows = self
            .client_mut()?
            .query(sql, &param_refs)
            .await
            .map_err(|e| DbError::DbError(e.to_string()))?;

        Ok(rows.iter().map(from_pg_row).collect())
    }

    async fn execute(&mut self, sql: &str, args: &[(String, Value)]) -> Result<u64> {
        let params: Vec<Box<dyn ToSql + Sync + Send>> =
            args.iter().map(|(_, v)| to_pg_param(v)).collect();
        let param_refs: Vec<&(dyn ToSql + Sync)> =
            params.iter().map(|v| &**v as &(dyn ToSql + Sync)).collect();

        let affected = self
            .client_mut()?
            .execute(sql, &param_refs)
            .await
            .map_err(|e| DbError::DbError(e.to_string()))?;
        Ok(affected)
    }

    async fn last_insert_id(&mut self) -> Result<u64> {
        let row = self
            .client_mut()?
            .query_opt("SELECT LASTVAL()", &[])
            .await
            .map_err(|e| DbError::DbError(e.to_string()))?;
        if let Some(row) = row {
            let id: i64 = row.try_get(0).unwrap_or(0);
            Ok(id.max(0) as u64)
        } else {
            Ok(0)
        }
    }

    async fn begin(&mut self) -> Result<()> {
        self.client_mut()?
            .batch_execute("BEGIN")
            .await
            .map_err(|e| DbError::DbError(e.to_string()))?;
        Ok(())
    }

    async fn commit(&mut self) -> Result<()> {
        self.client_mut()?
            .batch_execute("COMMIT")
            .await
            .map_err(|e| DbError::DbError(e.to_string()))?;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<()> {
        self.client_mut()?
            .batch_execute("ROLLBACK")
            .await
            .map_err(|e| DbError::DbError(e.to_string()))?;
        Ok(())
    }
}
