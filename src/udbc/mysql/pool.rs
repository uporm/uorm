use crate::Result;
use crate::error::DbError;
use crate::udbc::connection::Connection;
use crate::udbc::driver::Driver;
use crate::udbc::mysql::connection::MysqlConnection;
use crate::udbc::{DEFAULT_DB_NAME, PoolOptions};
use async_trait::async_trait;
use mysql_async::prelude::Queryable;
use mysql_async::{Opts, OptsBuilder, Pool, PoolConstraints, PoolOpts};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;

const MYSQL_TYPE: &str = "mysql";

/// `MysqlDriver` 负责管理 MySQL 连接池与配置。
///
/// 它实现 `Driver` trait 以提供数据库连接能力。
/// 该实现通过严格校验配置选项并优雅处理连接获取超时来保证正确性与健壮性。
pub struct MysqlDriver {
    url: String,
    name: String,
    options: Option<PoolOptions>,
    pool: Option<Pool>,
}

impl MysqlDriver {
    /// 使用给定连接 URL 创建新的 `MysqlDriver` 实例。
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            name: DEFAULT_DB_NAME.to_string(),
            url: url.into(),
            options: None,
            pool: None,
        }
    }

    /// 设置数据库驱动实例名称。
    pub fn name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    /// 配置连接选项（如连接池大小、超时）。
    /// 返回 `Self` 以支持链式调用。
    pub fn options(mut self, options: PoolOptions) -> Self {
        self.options = Some(options);
        self
    }

    /// 构建连接池并准备驱动使用。
    ///
    /// # 错误
    /// 在以下情况返回 `Error`：
    /// - 连接 URL 无效
    /// - 连接池约束无效（如 max_idle > max_open 或 max_open == 0）
    pub fn build(mut self) -> Result<Self> {
        let opts = Opts::from_url(&self.url).map_err(|e| {
            DbError::DbUrlError(format!("[{}] Invalid connection URL: {}", self.name, e))
        })?;

        let mut builder = OptsBuilder::from_opts(opts);

        // 默认启用 TCP keepalive（60s），避免空闲期间连接被断开
        // 这对电脑休眠或长时间空闲等场景很关键
        // 服务器或中间防火墙可能会悄然断开连接
        builder = builder.tcp_keepalive(Some(60_000u32));

        let url_params = parse_url_params(&self.url);

        if let Some(options) = self.options.as_mut() {
            if !url_params.is_empty() {
                options.extra_params.extend(url_params);
            }
        }

        if let Some(options) = &self.options {
            // 校验基本约束：max_open_conns 必须大于 0
            if options.max_open_conns == 0 {
                return Err(self.err_context(
                    "Invalid pool constraints: max_open_conns must be greater than 0",
                ));
            }

            // 配置连接池约束（最小/最大连接数）
            // mysql_async 要求：min <= max 且 max > 0
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

            // 如有设置则配置连接生命周期
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

    /// 带驱动名上下文的错误格式化辅助方法。
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
        // MySQL 使用 '?' 作为标准参数占位符
        "?".to_string()
    }

    async fn acquire(&self) -> Result<Box<dyn Connection>> {
        let pool = self.pool.as_ref().ok_or_else(|| {
            self.err_context("Connection pool not initialized (call build() first)")
        })?;

        let get_conn_fut = pool.get_conn();

        // 获取连接，可选超时
        let mut conn = if let Some(options) = &self.options {
            if options.timeout > 0 {
                // 为连接获取包裹超时
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

        if let Some(options) = &self.options {
            if !options.extra_params.is_empty() {
                for (key, value) in &options.extra_params {
                    if !is_valid_param_key(key) {
                        return Err(self.err_context(format!(
                            "Invalid extra_params key: {}",
                            key
                        )));
                    }
                    let stmt = format!("SET {} = ?", key);
                    conn.exec_drop(stmt, (value.as_str(),))
                        .await
                        .map_err(|e| {
                            self.err_context(format!("Failed to set {}: {}", key, e))
                        })?;
                }
            }
        }

        Ok(Box::new(MysqlConnection::new(conn)))
    }

    async fn close(&self) -> Result<()> {
        if let Some(pool) = &self.pool {
            // 优雅断开连接池
            // 由于 disconnect() 会消费句柄，因此克隆以通知共享连接池关闭
            pool.clone()
                .disconnect()
                .await
                .map_err(|e| self.err_context(format!("Failed to close pool: {}", e)))?;
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
