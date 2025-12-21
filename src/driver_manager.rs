use std::sync::{Arc, LazyLock};

use dashmap::DashMap;

use crate::error::DbError;
use crate::executor::mapper::Mapper;
use crate::executor::session::Session;
use crate::udbc::DEFAULT_DB_NAME;
use crate::udbc::driver::Driver;

// 全局单例（Rust 1.80+ 推荐）
pub static UORM: LazyLock<DriverManager> = LazyLock::new(DriverManager::new);

/// 数据库连接池管理器
/// Manages database connection pools
pub struct DriverManager {
    pools: DashMap<String, Arc<dyn Driver>>,
}

impl Default for DriverManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DriverManager {
    pub fn new() -> Self {
        Self {
            pools: DashMap::new(),
        }
    }

    /// 注册数据库连接池
    pub fn register(&self, driver: impl Driver + 'static) -> Result<(), DbError> {
        let name = driver.name().to_string();
        if name == DEFAULT_DB_NAME && self.pools.contains_key(&name) {
            return Err(DbError::General(format!(
                "Driver with name '{}' already registered",
                name
            )));
        }
        self.pools.insert(name, Arc::new(driver));
        Ok(())
    }

    /// 从指定模式加载 XML mapper 文件
    ///
    /// # 参数
    /// * `pattern` - 文件路径匹配模式，例如 "src/resources/**/*.xml"
    pub fn assets(&self, pattern: &str) -> Result<(), DbError> {
        crate::mapper_loader::load(pattern).map_err(|e| {
            DbError::General(format!("Failed to load mapper assets from pattern: {}", e))
        })
    }

    /// 获取用于执行原生 SQL 查询的客户端
    pub fn session(&self, db_name: &str) -> Option<Session> {
        self.pools
            .get(db_name)
            .map(|v| Session::new(v.value().clone()))
    }

    /// 获取用于执行映射器操作的客户端
    pub fn mapper(&self, db_name: &str) -> Option<Mapper> {
        self.pools
            .get(db_name)
            .map(|v| Mapper::new(v.value().clone()))
    }
}
