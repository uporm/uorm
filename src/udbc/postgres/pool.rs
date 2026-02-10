use async_trait::async_trait;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use std::collections::HashMap;
use std::str::FromStr;
use tokio::time::{Duration, timeout};
use tokio_postgres::{Config, NoTls};

use crate::Result;
use crate::error::DbError;
use crate::udbc::connection::Connection;
use crate::udbc::driver::Driver;
use crate::udbc::postgres::connection::PostgresConnection;
use crate::udbc::{DEFAULT_DB_NAME, PoolOptions};

const POSTGRES_TYPE: &str = "postgres";

pub struct PostgresDriver {
    url: String,
    name: String,
    options: Option<PoolOptions>,
    pool: Option<Pool>,
    extra_params: Option<HashMap<String, String>>,
}

impl PostgresDriver {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            name: DEFAULT_DB_NAME.to_string(),
            url: url.into(),
            options: None,
            pool: None,
            extra_params: None,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn options(mut self, options: PoolOptions) -> Self {
        self.options = Some(options);
        self
    }

    pub fn build(mut self) -> Result<Self> {
        let config = Config::from_str(&self.url).map_err(|e| {
            DbError::DbUrlError(format!("[{}] Invalid connection URL: {}", self.name, e))
        })?;

        let url_params = parse_url_params(&self.url);
        if let Some(options) = self.options.as_mut() {
            if !url_params.is_empty() {
                match options.extra_params.as_mut() {
                    Some(params) => params.extend(url_params),
                    None => options.extra_params = Some(url_params),
                }
            }
        }

        if let Some(options) = &self.options {
            if options.max_open_conns == 0 {
                return Err(self.err_context(
                    "Invalid pool constraints: max_open_conns must be greater than 0",
                ));
            }
            if options.max_idle_conns > options.max_open_conns {
                return Err(self.err_context(format!(
                    "Invalid pool constraints: max_idle_conns ({}) > max_open_conns ({})",
                    options.max_idle_conns, options.max_open_conns
                )));
            }
        }

        let mgr_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };
        let mgr = Manager::from_config(config, NoTls, mgr_config);
        let mut builder = Pool::builder(mgr);
        if let Some(options) = &self.options {
            builder = builder.max_size(options.max_open_conns as usize);
        }
        let pool = builder.build().map_err(|e| self.err_context(e))?;

        self.extra_params = self.options.as_ref().and_then(|o| o.extra_params.clone());
        self.pool = Some(pool);
        Ok(self)
    }

    fn err_context<T: std::fmt::Display>(&self, msg: T) -> DbError {
        DbError::DbError(format!("[{}] {}", self.name, msg))
    }
}

#[async_trait]
impl Driver for PostgresDriver {
    fn name(&self) -> &str {
        &self.name
    }

    fn r#type(&self) -> &str {
        POSTGRES_TYPE
    }

    fn placeholder(&self, param_seq: usize, _param_name: &str) -> String {
        format!("${}", param_seq)
    }

    async fn acquire(&self) -> Result<Box<dyn Connection>> {
        let pool = self.pool.as_ref().ok_or_else(|| {
            self.err_context("Connection pool not initialized (call build() first)")
        })?;

        let get_fut = pool.get();
        let client = if let Some(options) = &self.options {
            if options.timeout > 0 {
                match timeout(Duration::from_secs(options.timeout), get_fut).await {
                    Ok(result) => result,
                    Err(_) => {
                        return Err(self.err_context(format!(
                            "Connection acquisition timed out (timeout: {}s)",
                            options.timeout
                        )));
                    }
                }
            } else {
                get_fut.await
            }
        } else {
            get_fut.await
        }
        .map_err(|e| self.err_context(e))?;

        if let Some(extra_params) = &self.extra_params {
            for (key, value) in extra_params {
                if !is_valid_param_key(key) {
                    return Err(self.err_context(format!(
                        "Invalid extra_params key: {}",
                        key
                    )));
                }
                let stmt = format!("SET {} = $1", key);
                client
                    .execute(&stmt, &[value])
                    .await
                    .map_err(|e| {
                        self.err_context(format!("Failed to set {}: {}", key, e))
                    })?;
            }
        }

        Ok(Box::new(PostgresConnection::new(client)))
    }

    async fn close(&self) -> Result<()> {
        if let Some(pool) = &self.pool {
            pool.close();
        }
        Ok(())
    }
}

fn parse_url_params(url: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    let query = match url.split_once('?') {
        Some((_, query)) if !query.is_empty() => query,
        _ => return params,
    };

    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        let key = percent_decode(key);
        if key.is_empty() {
            continue;
        }
        let value = percent_decode(value);
        params.insert(key, value);
    }

    params
}

fn percent_decode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.as_bytes().iter().copied().peekable();
    while let Some(b) = chars.next() {
        match b {
            b'+' => out.push(' '),
            b'%' => {
                let hi = chars.next();
                let lo = chars.next();
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    if let (Some(hi), Some(lo)) = (hex_val(hi), hex_val(lo)) {
                        out.push((hi << 4 | lo) as char);
                    } else {
                        out.push('%');
                        out.push(hi as char);
                        out.push(lo as char);
                    }
                } else {
                    out.push('%');
                    if let Some(hi) = hi {
                        out.push(hi as char);
                    }
                    if let Some(lo) = lo {
                        out.push(lo as char);
                    }
                }
            }
            _ => out.push(b as char),
        }
    }
    out
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn is_valid_param_key(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}
