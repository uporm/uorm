# uorm

[![Crates.io](https://img.shields.io/crates/v/uorm)](https://crates.io/crates/uorm)
[![Documentation](https://docs.rs/uorm/badge.svg)](https://docs.rs/uorm)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Build Status](https://github.com/uporm/uorm/actions/workflows/ci.yml/badge.svg)](https://github.com/uporm/uorm/actions)

Rust 下的轻量级 ORM 框架，借鉴 Java MyBatis 的设计理念，强调 SQL 与业务逻辑分离：用 XML 管理 SQL，通过 `sql_id` 调用并把结果自动映射到 Rust 类型；同时支持直接执行原生 SQL，原生支持 `async/await`。

## 特性

- 🚀 **MyBatis 风格**：支持熟悉的 XML Mapper 语法，通过 `namespace.id` 唯一定位 SQL。
- 🎯 **动态 SQL**：支持 `<if>`、`<foreach>`、`<include>` 等标签，轻松构建复杂 SQL。
- 📦 **类型安全**：通过 `#[derive(Param)]` 自动处理参数绑定与结果映射。
- ⚡ **异步优先**：基于 `tokio` 运行时，全程支持 `async/await`，适配高并发场景。
- 🔧 **灵活配置**：支持多数据源管理、连接池优化、超时设置及事务控制。
- 🛠️ **过程宏增强**：提供 `#[sql]`、`#[uorm::transaction]` / `#[transaction]` 及 `mapper_assets!` 等宏，极大简化开发工作。
- 🗄️ **多数据库支持**：原生支持 SQLite 和 MySQL，架构易于扩展至其他 UDBC 驱动。
- 📝 **详细日志**：集成 `log` crate，提供 SQL 执行、耗时及参数详情，便于调试。

## 安装

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
uorm = "0.7.1"
```

### 特性开关 (Features)

- `sqlite`（默认开启）：支持 SQLite 数据库。
- `mysql`：支持 MySQL 数据库。

```toml
[dependencies]
# 仅启用 MySQL 支持
uorm = { version = "0.7.1", default-features = false, features = ["mysql"] }
```

## 快速开始

### 1) 注册数据库驱动

通过 `U` 全局单例注册驱动。`SqliteDriver` 和 `MysqlDriver` 均采用 Builder 模式。

```rust
use uorm::driver_manager::U;
use uorm::udbc::sqlite::pool::SqliteDriver;

#[tokio::main]
async fn main() -> uorm::Result<()> {
    // 创建驱动并指定名称（默认为 "default"）
    let driver = SqliteDriver::new("sqlite:./app.db")
        .build()?;
    
    // 注册到全局管理器
    U.register(driver)?;

    Ok(())
}
```

### 2) 加载 Mapper XML

`uorm` 提供两种方式加载 XML 资源：

**方式一：编译期内嵌（推荐）**
使用 `mapper_assets!` 宏，在编译时将 XML 文件内容嵌入二进制中，程序启动时自动注册。
在调试构建（debug）下会直接从磁盘按 glob 扫描加载，便于开发时热修改 XML。

```rust
use uorm::mapper_assets;

// 自动扫描路径下的所有 XML 文件并内嵌
mapper_assets!["resources/mappers"];
```

**方式二：运行时加载**

在程序启动后手动扫描文件系统加载 XML。

```rust
use uorm::driver_manager::U;

fn init_mappers() -> uorm::Result<()> {
    // 运行时扫描并解析 XML（glob 模式）
    U.assets("resources/mappers")?;
    Ok(())
}
```

### 3) 执行 Mapper 调用

```rust
use uorm::driver_manager::U;
use uorm::Param;

#[derive(Debug, Param)]
struct User {
    id: i64,
    name: String,
    age: i64,
}

#[derive(Param)]
struct IdArg {
    id: i64,
}

pub async fn get_user_by_id(user_id: i64) -> uorm::Result<Option<User>> {
    let mapper = U.mapper().expect("Driver not found");
    
    // execute 会根据 XML 定义的标签（select/insert/update/delete）自动执行。
    // 对于 select，如果结果只有一行且返回类型是结构体而非 Vec，会自动解包（Unwrap）。
    mapper.execute("user.get_by_id", &IdArg { id: user_id }).await
}
```

### 4) 基本类型返回 (Scalar Return)

除了返回结构体或 `Vec`，`execute` 也支持直接返回基本类型（如 `i64`, `String`, `f64` 等）及其 `Option` 包装。适用于 `count(*)`、`max(column)` 等聚合查询。

```rust
pub async fn scalar_example() -> uorm::Result<()> {
    let mapper = U.mapper().expect("Driver not found");

    // 返回单个整数 (例如 count(*))
    let count: i64 = mapper.execute("user.count", &()).await?;

    // 返回单个字符串 (例如 max(name))
    let name: String = mapper.execute("user.max_name", &()).await?;

    // 返回 Option (处理可能为空的结果)
    let max_id: Option<i64> = mapper.execute("user.max_id", &()).await?;
    
    Ok(())
}
```

## SQL 过程宏 (`#[sql]`)

使用 `#[sql]` 宏可以像定义 DAO 接口一样操作数据库，代码更加优雅。

```rust
use uorm::{sql, Param, Result};

#[derive(Debug, Param)]
struct User {
    id: i64,
    name: String,
}

#[sql("user")] // 指定 XML 的 namespace
struct UserDao;

impl UserDao {
    #[sql("get_by_id")] // 对应 user.get_by_id
    pub async fn get(id: i64) -> Result<Option<User>> {
        // exec!() 是由 #[sql] 宏在函数内部注入的局部宏
        // 它会自动捕获函数参数、namespace 和 id 并执行调用
        exec!() 
    }

    #[sql(id = "list_all", database = "other_db")] // 可指定特定的数据库名称
    pub async fn list_all() -> Result<Vec<User>> {
        exec!()
    }
}
```

## 直接执行 SQL (`Session`)

如果不想使用 XML，也可以通过 `Session` 直接执行带有命名参数的 SQL。`uorm` 内部集成了轻量级模板引擎。

```rust
use uorm::driver_manager::U;
use uorm::Param;

#[derive(Param)]
struct UserParam {
    name: String,
    age: i32,
}

pub async fn add_user() -> uorm::Result<u64> {
    let session = U.session().expect("Default driver not found");

    // 支持 #{field} 语法绑定参数，内部会自动处理 SQL 注入防护
    let affected = session.execute(
        "INSERT INTO users(name, age) VALUES (#{name}, #{age})",
        &UserParam { name: "Alice".to_string(), age: 18 }
    ).await?;

    Ok(affected)
}
```

## 事务管理

### 自动事务宏 (`#[uorm::transaction]`)

使用 `#[transaction]` 宏可以简化事务代码：它会在执行函数体前尝试开启事务，当函数返回 `Ok(_)` 时提交事务（`commit()`），返回 `Err(_)` 时回滚事务（`rollback()`）。如果当前线程已存在同库的事务上下文（例如嵌套调用），宏不会重复开启/提交事务。
该宏要求被标注的函数返回 `Result<T, E>`，并且 `E` 能从 `uorm::error::Error` 转换（即满足 `E: From<Error>`），以便将 `begin/commit` 的错误向外返回。注意：回滚失败会被忽略并优先返回原始业务错误。

另外，事务上下文基于线程局部存储（TLS）。在 tokio 多线程运行时下，任务可能跨线程恢复执行；如需严格保证同一事务内共享同一连接，请使用单线程运行时或确保任务固定在线程上执行。

```rust
use uorm::driver_manager::U;
use uorm::Param;

#[derive(Param)]
struct MyData {
    id: i64,
    name: String,
}

#[uorm::transaction]
async fn transfer_data(data: &MyData) -> uorm::Result<()> {
    let session = U.session().expect("Default driver not found");
    session
        .execute("INSERT INTO t(id, name) VALUES (#{id}, #{name})", data)
        .await?;
    session
        .execute("UPDATE t SET name = #{name} WHERE id = #{id}", data)
        .await?;
    Ok(())
}

#[derive(Param)]
struct IdArg {
    id: i64,
}

#[uorm::transaction(database = "other_db")]
async fn custom_session_name() -> uorm::Result<()> {
    let session = U.session_by_name("other_db").expect("Database driver not found");
    session.execute("DELETE FROM t WHERE id = #{id}", &IdArg { id: 1 })
        .await?;
    Ok(())
}
```

### 手动管理事务

`uorm` 使用线程局部存储（Thread Local Storage）管理事务上下文，确保在同一线程内的操作共享同一个事务连接。

```rust
use uorm::driver_manager::U;

async fn manual_transaction() -> uorm::Result<()> {
    let session = U.session().expect("Default driver not found");
    session.begin().await?;

    match do_work(&session).await {
        Ok(_) => session.commit().await?,
        Err(e) => {
            session.rollback().await?;
            return Err(e);
        }
    }
    Ok(())
}
```

## XML Mapper 示例

```mapper
<mapper namespace="user">
  <!-- 基本查询 -->
  <select id="get_by_id">
    SELECT id, name, age FROM users WHERE id = #{id}
  </select>

  <!-- 动态 SQL：if 标签 -->
  <select id="search">
    SELECT * FROM users
    <where>
      <if test="name != null">
        AND name LIKE #{name}
      </if>
      <if test="min_age != null">
        AND age >= #{min_age}
      </if>
    </where>
  </select>

  <!-- 动态 SQL：foreach 标签 -->
  <select id="list_by_ids">
    SELECT * FROM users
    WHERE id IN
    <foreach item="id" collection="ids" open="(" separator="," close=")">
      #{id}
    </foreach>
  </select>

  <!-- 插入并获取自增 ID -->
  <!-- 当 returnKey 为 true 时，execute 将返回最后插入的 ID -->
  <insert id="insert_user" returnKey="true">
    INSERT INTO users(name, age) VALUES (#{name}, #{age})
  </insert>
</mapper>
```

## 高级配置

### 连接池与超时

```rust
use uorm::udbc::PoolOptions;
use uorm::udbc::mysql::pool::MysqlDriver;

fn build_mysql_driver() -> uorm::Result<MysqlDriver> {
    let options = PoolOptions {
        max_open_conns: 20,
        max_idle_conns: 5,
        max_lifetime: 3600,
        timeout: 5, // 连接获取超时（秒）
    };

    MysqlDriver::new("mysql://user:pass@localhost/db")
        .options(options)
        .build()
}
```

### SQLite 特殊说明

- **并发性**：SQLite 驱动默认开启了 `WAL` 模式（Write-Ahead Logging）和 `foreign_keys` 支持，显著提升并发读写性能。
- **内存数据库**：使用 `sqlite::memory:` 或 `sqlite://:memory:`。注意：当前 SQLite 驱动每次 `acquire()` 都会创建新连接；对 `:memory:` 而言，这意味着每次都是全新的空库。需要共享状态时建议使用文件数据库，或使用 SQLite URI 共享内存（例如 `sqlite:file:app?mode=memory&cache=shared`）。

## 日志监控

`uorm` 使用 `log` crate 输出详细的执行日志。

```rust
fn init_logging() {
    // 建议在开发环境下开启 Debug 级别
    // 输出示例:
    // DEBUG [uorm] Query: sql=SELECT ... WHERE id = ?, params=[("id", I64(1))], elapsed=2ms, rows=1
    env_logger::Builder::new().filter_level(log::LevelFilter::Debug).init();
}
```

## License

Apache-2.0
