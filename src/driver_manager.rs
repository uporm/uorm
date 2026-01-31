use std::sync::{Arc, LazyLock};

use dashmap::DashMap;

use crate::Result;
use crate::error::DbError;
use crate::executor::mapper::Mapper;
use crate::executor::session::Session;
use crate::udbc::DEFAULT_DB_NAME;
use crate::udbc::driver::Driver;

/// `uorm` 库的全局入口。
/// 使用该单例注册驱动、加载 mapper 资源，并创建 session 或 mapper。
pub static U: LazyLock<DriverManager> = LazyLock::new(DriverManager::new);

/// 数据库驱动及其连接池的管理器。
///
/// `DriverManager` 作为注册表，用唯一名称注册不同数据库驱动（MySQL、SQLite 等），
/// 并提供创建 `Session` 与 `Mapper` 的方法以访问已注册的数据库。
pub struct DriverManager {
    /// 线程安全的 map，按唯一名称保存已注册的数据库驱动。
    pools: DashMap<String, Arc<dyn Driver>>,
}

impl Default for DriverManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DriverManager {
    /// 创建一个新的空 `DriverManager`。
    pub fn new() -> Self {
        Self {
            pools: DashMap::new(),
        }
    }

    /// 向管理器注册数据库驱动。
    ///
    /// 驱动名称（通过 `driver.name()` 获取）用作注册 key。
    ///
    /// # 错误
    /// 若已有同名驱动（尤其是默认名）被注册，则返回错误。
    pub fn register(&self, driver: impl Driver + 'static) -> Result<()> {
        let name = driver.name().to_string();
        if name == DEFAULT_DB_NAME && self.pools.contains_key(&name) {
            return Err(DbError::DriverError(format!(
                "Driver with name '{}' already registered",
                name
            )));
        }
        self.pools.insert(name, Arc::new(driver));
        Ok(())
    }

    /// 根据 glob 模式从文件系统加载 XML mapper 文件。
    ///
    /// 该方法用于注册 XML 中定义的 SQL 模板。
    ///
    /// # 参数
    /// * `pattern` - 用于查找 mapper 文件的 glob 模式（如 "resources/mappers/*.xml"）。
    pub fn assets(&self, pattern: &str) -> Result<()> {
        crate::mapper_loader::load(pattern).map_err(|e| {
            DbError::MapperLoadError(format!("Failed to load mapper assets from pattern: {}", e))
        })
    }

    /// 为默认数据库创建 `Session`。
    ///
    /// # 返回
    /// 若默认驱动已注册则返回 `Some(Session)`，否则返回 `None`。
    pub fn session(&self) -> Option<Session> {
        self.session_by_name(DEFAULT_DB_NAME)
    }

    /// 为指定名称的数据库创建 `Session`。
    ///
    /// `Session` 用于执行原始 SQL 查询与管理事务。
    ///
    /// # 返回
    /// 若 `db_name` 对应驱动已注册则返回 `Some(Session)`，否则返回 `None`。
    pub fn session_by_name(&self, db_name: &str) -> Option<Session> {
        self.pools
            .get(db_name)
            .map(|v| Session::new(v.value().clone()))
    }

    /// 为默认数据库创建 `Mapper`。
    ///
    /// # 返回
    /// 若默认驱动已注册则返回 `Some(Mapper)`，否则返回 `None`。
    pub fn mapper(&self) -> Option<Mapper> {
        self.mapper_by_name(DEFAULT_DB_NAME)
    }

    /// 为指定名称的数据库创建 `Mapper`。
    ///
    /// `Mapper` 用于按 ID 执行 XML 中定义的 SQL 语句。
    ///
    /// # 返回
    /// 若 `db_name` 对应驱动已注册则返回 `Some(Mapper)`，否则返回 `None`。
    pub fn mapper_by_name(&self, db_name: &str) -> Option<Mapper> {
        self.pools
            .get(db_name)
            .map(|v| Mapper::new(v.value().clone()))
    }
}
