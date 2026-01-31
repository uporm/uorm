use crate::Result;
use crate::error::DbError;
use crate::executor::exec::{execute_conn, map_rows, query_conn};
use crate::udbc::connection::Connection;
use crate::udbc::driver::Driver;
use crate::udbc::value::{FromValue, ToValue, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::Mutex;

type TransactionContextMap = HashMap<String, Arc<Mutex<TransactionContext>>>;
const TX_CONN_CLOSED: &str = "Transaction connection closed";

tokio::task_local! {
    static TX_CONTEXT: RefCell<TransactionContextMap>;
}

fn inline_template_name(sql: &str) -> String {
    let mut hasher = DefaultHasher::new();
    sql.hash(&mut hasher);
    format!("__inline__:{:x}", hasher.finish())
}

struct TransactionContext {
    conn: Option<Box<dyn Connection>>,
    committed: bool,
}

impl TransactionContext {
    async fn begin(pool: Arc<dyn Driver>) -> Result<Self> {
        let mut conn: Box<dyn Connection> = pool.acquire().await?;
        conn.begin().await?;
        Ok(Self {
            conn: Some(conn),
            committed: false,
        })
    }

    async fn commit(&mut self) -> Result<()> {
        if let Some(conn) = self.conn.as_mut() {
            conn.commit().await?;
        }
        self.committed = true;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<()> {
        let r = if let Some(conn) = self.conn.as_mut() {
            conn.rollback().await
        } else {
            Ok(())
        };
        if r.is_ok() {
            self.committed = true;
        }
        r
    }

    fn connection_mut(&mut self) -> Option<&mut Box<dyn Connection>> {
        self.conn.as_mut()
    }
}

impl Drop for TransactionContext {
    fn drop(&mut self) {
        if !self.committed
            && let Some(mut conn) = self.conn.take()
        {
            tokio::spawn(async move {
                let _ = conn.rollback().await;
            });
        }
    }
}

pub async fn with_tx_context<F, Fut, R>(f: F) -> R
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = R>,
{
    if TX_CONTEXT.try_with(|_| ()).is_ok() {
        f().await
    } else {
        TX_CONTEXT
            .scope(RefCell::new(HashMap::new()), f())
            .await
    }
}

fn get_tx_context(key: &str) -> Option<Arc<Mutex<TransactionContext>>> {
    TX_CONTEXT
        .try_with(|map| map.borrow().get(key).cloned())
        .ok()
        .flatten()
}

fn remove_tx_context(key: &str) {
    let _ = TX_CONTEXT.try_with(|map| {
        map.borrow_mut().remove(key);
    });
}

/// 管理连接池与事务状态的数据库会话封装。
///
/// 提供统一接口，无论是否处于事务中都可执行查询。
pub struct Session {
    pool: Arc<dyn Driver>,
}

pub trait TransactionResult: Sized {
    fn is_ok(&self) -> bool;
    fn from_db_error(err: DbError) -> Self;
}

impl<T, E> TransactionResult for std::result::Result<T, E>
where
    E: From<DbError>,
{
    fn is_ok(&self) -> bool {
        self.is_ok()
    }

    fn from_db_error(err: DbError) -> Self {
        Err(err.into())
    }
}

impl Session {
    pub fn new(pool: Arc<dyn Driver>) -> Self {
        Self { pool }
    }

    fn tx_context(&self) -> Option<Arc<Mutex<TransactionContext>>> {
        get_tx_context(self.pool.name())
    }

    /// 为当前数据库连接开启新事务。
    ///
    /// 事务状态存放在任务本地的 map（`TX_CONTEXT`）中，以驱动名作为 key。
    /// 这样同一任务内的嵌套或后续调用都能访问当前事务。
    ///
    /// # 错误
    /// 若当前线程已为该驱动开启事务，则返回 `Error`。
    pub async fn begin(&self) -> Result<()> {
        let key = self.pool.name().to_string();
        if self.tx_context().is_some() {
            return Err(DbError::DbError(format!(
                "Transaction already started for '{}'",
                key
            )));
        }

        let ctx = TransactionContext::begin(self.pool.clone()).await?;
        TX_CONTEXT
            .try_with(|tx| {
                tx.borrow_mut().insert(key, Arc::new(Mutex::new(ctx)));
            })
            .map_err(|_| DbError::DbError("Transaction context not initialized".to_string()))?;
        Ok(())
    }

    /// 提交当前数据库连接上的活动事务。
    ///
    /// 若没有活动事务，该方法不做任何事并返回 `Ok(())`。
    /// 完成后会从任务本地存储中移除事务上下文。
    pub async fn commit(&self) -> Result<()> {
        let key = self.pool.name().to_string();
        let tx = self.tx_context();
        let Some(tx) = tx else {
            return Ok(());
        };

        {
            let mut ctx = tx.lock().await;
            ctx.commit().await?;
        }

        remove_tx_context(&key);
        Ok(())
    }

    /// 回滚当前数据库连接上的活动事务。
    ///
    /// 若没有活动事务，该方法不做任何事并返回 `Ok(())`。
    /// 完成后会从任务本地存储中移除事务上下文。
    pub async fn rollback(&self) -> Result<()> {
        let key = self.pool.name().to_string();
        let tx = self.tx_context();
        let Some(tx) = tx else {
            return Ok(());
        };

        {
            let mut ctx = tx.lock().await;
            ctx.rollback().await?;
        }

        remove_tx_context(&key);
        Ok(())
    }

    pub fn is_transaction_active(&self) -> bool {
        self.tx_context().is_some()
    }

    /// 执行修改数据的 SQL 语句（如 INSERT、UPDATE、DELETE）。
    ///
    /// # 参数
    /// * `sql` - 要执行的 SQL 模板。
    /// * `args` - 绑定到 SQL 模板的参数。
    ///
    /// # 返回
    /// 受影响的行数。
    ///
    /// 该方法会自动判断是否在活动事务内运行。
    /// 若是，则委托给事务上下文执行；否则渲染模板并直接使用连接池中的连接执行。
    pub async fn execute<T>(&self, sql: &str, args: &T) -> Result<u64>
    where
        T: ToValue,
    {
        let template_name = inline_template_name(sql);
        self.execute_named(&template_name, sql, args).await
    }

    pub async fn execute_named<T>(&self, template_name: &str, sql: &str, args: &T) -> Result<u64>
    where
        T: ToValue,
    {
        if let Some(tx) = self.tx_context() {
            let mut ctx = tx.lock().await;
            if let Some(conn) = ctx.connection_mut() {
                return execute_conn(conn.as_mut(), self.pool.as_ref(), template_name, sql, args)
                    .await;
            } else {
                return Err(DbError::DbError(TX_CONN_CLOSED.to_string()));
            }
        }

        let mut conn: Box<dyn Connection> = self.pool.acquire().await?;
        execute_conn(conn.as_mut(), self.pool.as_ref(), template_name, sql, args).await
    }

    /// 执行 SQL 查询并将结果行映射为类型 `R` 的集合。
    ///
    /// # 参数
    /// * `sql` - 要执行的 SQL 模板。
    /// * `args` - 绑定到 SQL 模板的参数。
    ///
    /// # 返回
    /// 包含反序列化结果的 `Vec<R>`。
    pub async fn query<R, T>(&self, sql: &str, args: &T) -> Result<Vec<R>>
    where
        T: ToValue,
        R: FromValue,
    {
        let rows = self.query_raw(sql, args).await?;
        map_rows(rows)
    }

    /// 执行 SQL 查询并以原始 HashMap 列表返回结果。
    ///
    /// 每个 HashMap 代表一行，键为列名，值为列值。
    pub async fn query_raw<T>(&self, sql: &str, args: &T) -> Result<Vec<HashMap<String, Value>>>
    where
        T: ToValue,
    {
        let template_name = inline_template_name(sql);
        self.query_raw_named(&template_name, sql, args).await
    }

    pub async fn query_raw_named<T>(
        &self,
        template_name: &str,
        sql: &str,
        args: &T,
    ) -> Result<Vec<HashMap<String, Value>>>
    where
        T: ToValue,
    {
        if let Some(tx) = self.tx_context() {
            let mut ctx = tx.lock().await;
            if let Some(conn) = ctx.connection_mut() {
                return query_conn(conn.as_mut(), self.pool.as_ref(), template_name, sql, args)
                    .await;
            } else {
                return Err(DbError::DbError(TX_CONN_CLOSED.to_string()));
            }
        }

        let mut conn: Box<dyn Connection> = self.pool.acquire().await?;
        query_conn(conn.as_mut(), self.pool.as_ref(), template_name, sql, args).await
    }

    /// 获取最后一次插入的行 ID。
    pub async fn last_insert_id(&self) -> Result<u64> {
        if let Some(tx) = self.tx_context() {
            let mut ctx = tx.lock().await;
            if let Some(conn) = ctx.connection_mut() {
                return conn.last_insert_id().await;
            } else {
                return Err(DbError::DbError(TX_CONN_CLOSED.to_string()));
            }
        }

        let mut conn: Box<dyn Connection> = self.pool.acquire().await?;
        conn.last_insert_id().await
    }
}
