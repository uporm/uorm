use crate::Result;
use crate::udbc::connection::Connection;
use async_trait::async_trait;

/// `Driver` 定义数据库驱动的通用接口。
///
/// 驱动负责：
/// - 提供自身元数据（名称、类型）
/// - 生成 SQL 查询的参数占位符
/// - 管理数据库连接
/// - 在关闭时释放资源
#[async_trait]
pub trait Driver: Send + Sync {
    /// 返回驱动名称。
    ///
    /// 示例："postgres"、"mysql"、"sqlite"
    fn name(&self) -> &str;

    /// 返回驱动类型。
    ///
    /// 可用于区分不同数据库类别或协议。
    fn r#type(&self) -> &str;

    /// 生成查询参数的占位符字符串。
    ///
    /// # 参数
    /// * `param_seq` - 参数序号（从 1 开始）
    /// * `param_name` - 参数逻辑名称
    ///
    /// # 返回
    /// 数据库相关的占位符字符串。
    ///
    /// 示例输出：
    /// - PostgreSQL：`$1`
    /// - MySQL / SQLite：`?`
    /// - 命名参数：`:param_name`
    fn placeholder(&self, param_seq: usize, param_name: &str) -> String;

    /// 创建并返回新的数据库连接。
    ///
    /// # 返回
    /// - `Ok(Box<dyn Connection>)`：连接建立成功
    /// - `Err(Error)`：连接创建失败
    async fn acquire(&self) -> Result<Box<dyn Connection>>;

    /// 关闭驱动并释放相关资源。
    ///
    /// 驱动不再需要时应调用该方法。
    ///
    /// # 返回
    /// - `Ok(())`：清理成功
    /// - `Err(Error)`：清理过程发生错误
    async fn close(&self) -> Result<()>;
}
