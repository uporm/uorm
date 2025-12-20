use crate::error::DbError;
use crate::udbc::connection::Connection;
use crate::udbc::driver::Driver;
use crate::udbc::{ConnectionOptions, DEFAULT_DB_NAME};
use crate::udbc_mysql::connection::MysqlConnection;
use async_trait::async_trait;
use mysql_async::Pool as MySqlPoolInternal;
use mysql_async::{Opts, OptsBuilder, PoolConstraints, PoolOpts};
use std::sync::Arc;
use std::time::Duration;

const MYSQL_TYPE: &str = "mysql";

pub struct MysqlDriver {
    url: String,
    name: String,
    r#type: String,
    options: Option<ConnectionOptions>,
    pool: Option<MySqlPoolInternal>,
}

impl MysqlDriver {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            name: DEFAULT_DB_NAME.to_string(),
            r#type: MYSQL_TYPE.to_string(),
            url: url.into(),
            options: None,
            pool: None,
        }
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    pub fn options(mut self, options: ConnectionOptions) {
        self.options = Some(options);
    }

    pub fn build(mut self) -> Result<Self, DbError> {
        let opts = Opts::from_url(&self.url).map_err(|e| DbError::Database(e.to_string()))?;
        let mut builder = OptsBuilder::from_opts(opts);

        if let Some(options) = &self.options {
            let constraints = PoolConstraints::new(
                options.max_idle_conns as usize,
                options.max_open_conns as usize,
            )
            .ok_or_else(|| DbError::Database("Invalid pool constraints: min > max".to_string()))?;

            let mut pool_opts = PoolOpts::default().with_constraints(constraints);

            if options.max_lifetime > 0 {
                pool_opts = pool_opts
                    .with_inactive_connection_ttl(Duration::from_secs(options.max_lifetime));
            }

            builder = builder.pool_opts(pool_opts);
        }

        let pool = MySqlPoolInternal::new(builder);
        self.pool = Some(pool);
        Ok(self)
    }
}

#[async_trait]
impl Driver for MysqlDriver {
    fn name(&self) -> &str {
        &self.name
    }

    fn r#type(&self) -> &str {
        &self.r#type
    }

    fn placeholder(&self, _param_seq: usize, _param_name: &str) -> String {
        "?".to_string()
    }

    async fn connection(&self) -> Result<Arc<dyn Connection>, DbError> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| DbError::Database("Pool not initialized".to_string()))?;
        let conn = pool
            .get_conn()
            .await
            .map_err(|e| DbError::Database(e.to_string()))?;
        Ok(Arc::new(MysqlConnection::new(conn)))
    }

    async fn close(&self) -> Result<(), DbError> {
        if let Some(pool) = &self.pool {
            pool.clone()
                .disconnect()
                .await
                .map_err(|e| DbError::Database(e.to_string()))?;
        }
        Ok(())
    }
}
