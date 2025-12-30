pub mod connection;
pub mod driver;
#[cfg(feature = "mysql")]
pub mod mysql;
#[cfg(feature = "sqlite")]
pub mod sqlite;
pub mod value;

pub use value::Value;

pub const DEFAULT_DB_NAME: &str = "default";

pub struct PoolOptions {
    pub max_open_conns: u64, // Set the maximum number of connections in the pool
    pub max_idle_conns: u64, // Set the maximum number of idle connections in the pool
    pub max_lifetime: u64,   // Set the maximum lifetime of a connection
    pub timeout: u64,        // Set the timeout for getting a connection from the pool
}
