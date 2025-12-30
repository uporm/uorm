use crate::Result;
use crate::error::DbError;
use crate::udbc::connection::Connection;
use crate::udbc::driver::Driver;
use crate::udbc::mysql::connection::MysqlConnection;
use crate::udbc::{DEFAULT_DB_NAME, PoolOptions};
use async_trait::async_trait;
use mysql_async::{Opts, OptsBuilder, Pool, PoolConstraints, PoolOpts};
use std::time::Duration;
use tokio::time::timeout;

const MYSQL_TYPE: &str = "mysql";

/// `MysqlDriver` manages the MySQL connection pool and configuration.
///
/// It implements the `Driver` trait to provide database connectivity.
/// This implementation prioritizes correctness and robustness by strictly validating
/// configuration options and handling connection acquisition timeouts gracefully.
pub struct MysqlDriver {
    url: String,
    name: String,
    options: Option<PoolOptions>,
    pool: Option<Pool>,
}

impl MysqlDriver {
    /// Creates a new `MysqlDriver` instance with the given connection URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            name: DEFAULT_DB_NAME.to_string(),
            url: url.into(),
            options: None,
            pool: None,
        }
    }

    /// Sets the name of the database driver instance.
    pub fn name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    /// Configures the connection options (e.g., pool size, timeout).
    /// Returns `Self` to allow method chaining.
    pub fn options(mut self, options: PoolOptions) -> Self {
        self.options = Some(options);
        self
    }

    /// Builds the connection pool and prepares the driver for use.
    ///
    /// # Errors
    /// Returns `Error` if:
    /// - The connection URL is invalid.
    /// - Pool constraints are invalid (e.g., max_idle > max_open or max_open == 0).
    pub fn build(mut self) -> Result<Self> {
        let opts = Opts::from_url(&self.url).map_err(|e| {
            DbError::DbUrlError(format!("[{}] Invalid connection URL: {}", self.name, e))
        })?;

        let mut builder = OptsBuilder::from_opts(opts);

        if let Some(options) = &self.options {
            // Validate basic constraints: max_open_conns must be > 0
            if options.max_open_conns == 0 {
                return Err(self.err_context(
                    "Invalid pool constraints: max_open_conns must be greater than 0",
                ));
            }

            // Configure connection pool constraints (min/max connections)
            // mysql_async requires: min <= max and max > 0
            let constraints = PoolConstraints::new(
                options.max_idle_conns as usize,
                options.max_open_conns as usize,
            )
            .ok_or_else(|| {
                self.err_context(format!(
                    "Invalid pool constraints: max_idle_conns ({}) > max_open_conns ({})",
                    options.max_idle_conns, options.max_open_conns
                ))
            })?;

            let mut pool_opts = PoolOpts::default().with_constraints(constraints);

            // Configure connection lifetime if specified
            if options.max_lifetime > 0 {
                pool_opts = pool_opts
                    .with_inactive_connection_ttl(Duration::from_secs(options.max_lifetime));
            }

            builder = builder.pool_opts(pool_opts);
        }

        let pool = Pool::new(builder);
        self.pool = Some(pool);
        Ok(self)
    }

    /// Helper to format errors with the driver name context.
    fn err_context<T: std::fmt::Display>(&self, msg: T) -> DbError {
        DbError::DbError(format!("[{}] {}", self.name, msg))
    }
}

#[async_trait]
impl Driver for MysqlDriver {
    fn name(&self) -> &str {
        &self.name
    }

    fn r#type(&self) -> &str {
        MYSQL_TYPE
    }

    fn placeholder(&self, _param_seq: usize, _param_name: &str) -> String {
        // MySQL uses '?' as the standard parameter placeholder
        "?".to_string()
    }

    async fn acquire(&self) -> Result<Box<dyn Connection>> {
        let pool = self.pool.as_ref().ok_or_else(|| {
            self.err_context("Connection pool not initialized (call build() first)")
        })?;

        let get_conn_fut = pool.get_conn();

        // Acquire a connection, optionally with a timeout
        let conn = if let Some(options) = &self.options {
            if options.timeout > 0 {
                // Wrap acquisition in a timeout
                match timeout(Duration::from_secs(options.timeout), get_conn_fut).await {
                    Ok(result) => result,
                    Err(_) => {
                        return Err(self.err_context(format!(
                            "Connection acquisition timed out (timeout: {}s)",
                            options.timeout
                        )));
                    }
                }
            } else {
                get_conn_fut.await
            }
        } else {
            get_conn_fut.await
        }
        .map_err(|e| self.err_context(e))?;

        Ok(Box::new(MysqlConnection::new(conn)))
    }

    async fn close(&self) -> Result<()> {
        if let Some(pool) = &self.pool {
            // Gracefully disconnect the pool.
            // We clone the pool handle because disconnect() consumes it,
            // but we want to signal the shared pool to close.
            pool.clone()
                .disconnect()
                .await
                .map_err(|e| self.err_context(format!("Failed to close pool: {}", e)))?;
        }
        Ok(())
    }
}
