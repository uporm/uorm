use std::sync::{Arc, LazyLock};

use dashmap::DashMap;

use crate::error::DbError;
use crate::Result;
use crate::executor::mapper::Mapper;
use crate::executor::session::Session;
use crate::udbc::DEFAULT_DB_NAME;
use crate::udbc::driver::Driver;

/// The global entry point for the `uorm` library.
/// Use this singleton to register drivers, load mapper assets, and create sessions or mappers.
pub static U: LazyLock<DriverManager> = LazyLock::new(DriverManager::new);

/// A manager for database drivers and their associated connection pools.
///
/// `DriverManager` acts as a registry where different database drivers (MySQL, SQLite, etc.)
/// can be registered under unique names. It also provides methods to create `Session`
/// and `Mapper` instances for interacting with the registered databases.
pub struct DriverManager {
    /// A thread-safe map storing registered database drivers by their unique names.
    pools: DashMap<String, Arc<dyn Driver>>,
}

impl Default for DriverManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DriverManager {
    /// Creates a new, empty `DriverManager`.
    pub fn new() -> Self {
        Self {
            pools: DashMap::new(),
        }
    }

    /// Registers a database driver with the manager.
    ///
    /// The driver's name (retrieved via `driver.name()`) is used as the registration key.
    ///
    /// # Errors
    /// Returns an error if a driver with the same name (especially the default name)
    /// is already registered.
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

    /// Loads XML mapper files from the file system based on a glob pattern.
    ///
    /// This method allows you to register SQL templates defined in XML files.
    ///
    /// # Arguments
    /// * `pattern` - A glob pattern (e.g., "resources/mappers/*.xml") to find mapper files.
    pub fn assets(&self, pattern: &str) -> Result<()> {
        crate::mapper_loader::load(pattern).map_err(|e| {
            DbError::MapperLoadError(format!("Failed to load mapper assets from pattern: {}", e))
        })
    }

    /// Creates a `Session` for the default database.
    ///
    /// # Returns
    /// `Some(Session)` if the default driver is registered, otherwise `None`.
    pub fn session(&self) -> Option<Session> {
        self.session_by_name(DEFAULT_DB_NAME)
    }

    /// Creates a `Session` for the specified database by name.
    ///
    /// A `Session` is used for executing raw SQL queries and managing transactions.
    ///
    /// # Returns
    /// `Some(Session)` if a driver with `db_name` is registered, otherwise `None`.
    pub fn session_by_name(&self, db_name: &str) -> Option<Session> {
        self.pools
            .get(db_name)
            .map(|v| Session::new(v.value().clone()))
    }

    /// Creates a `Mapper` for the default database.
    ///
    /// # Returns
    /// `Some(Mapper)` if the default driver is registered, otherwise `None`.
    pub fn mapper(&self) -> Option<Mapper> {
        self.mapper_by_name(DEFAULT_DB_NAME)
    }

    /// Creates a `Mapper` for the specified database by name.
    ///
    /// A `Mapper` is used for executing SQL statements defined in XML files by their IDs.
    ///
    /// # Returns
    /// `Some(Mapper)` if a driver with `db_name` is registered, otherwise `None`.
    pub fn mapper_by_name(&self, db_name: &str) -> Option<Mapper> {
        self.pools
            .get(db_name)
            .map(|v| Mapper::new(v.value().clone()))
    }
}
