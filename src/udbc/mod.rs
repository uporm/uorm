pub mod value;

pub mod connection;
pub mod deserializer;
pub mod driver;
pub mod serializer;

pub const DEFAULT_DB_NAME: &'static str = "default";

pub struct ConnectionOptions {
    pub max_open_conns: u64, // 设置池最大连接数
    pub max_idle_conns: u64, // 设置池最大空闲数
    pub max_lifetime: u64,   // 设置连接最大生命周期
    pub timeout: u64,        // 设置连接池获取连接的超时时间
}
