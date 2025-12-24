# uorm

[![Crates.io](https://img.shields.io/crates/v/uorm)](https://crates.io/crates/uorm)
[![Documentation](https://docs.rs/uorm/badge.svg)](https://docs.rs/uorm)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Build Status](https://github.com/uporm/uorm/actions/workflows/ci.yml/badge.svg)](https://github.com/uporm/uorm/actions)

Rust 下的轻量级 ORM 框架，借鉴 Java MyBatis 的设计理念，强调 SQL 与业务逻辑分离：用 XML 管理 SQL，通过 `sql_id` 调用并把结果自动反序列化到 Rust 类型；同时支持直接执行原生 SQL，原生支持 `async/await`。

## 特性

- 🚀 **MyBatis 风格**：支持熟悉的 XML Mapper 语法，通过 `namespace.id` 唯一定位 SQL。
- 🎯 **动态 SQL**：支持 `<if>`、`<foreach>`、`<include>` 等标签，轻松构建复杂 SQL。
- 📦 **类型安全**：利用 Rust 强大的类型系统，通过 `serde` 自动处理参数绑定和结果反序列化。
- ⚡ **异步优先**：基于 `tokio` 运行时，全程支持 `async/await`，适配高并发场景。
- 🔧 **灵活配置**：支持多数据源管理、连接池优化、超时设置及事务控制。
- 🛠️ **过程宏增强**：提供 `#[sql]`、`#[transaction]` 及 `mapper_assets!` 等宏，极大简化开发工作。
- 🗄️ **多数据库支持**：原生支持 SQLite 和 MySQL，架构易于扩展至其他 UDBC 驱动。
- 📝 **详细日志**：集成 `log` crate，提供 SQL 执行、耗时及参数详情，便于调试。

## 安装

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
uorm = "0.4.1"
```

### 特性开关 (Features)

- `sqlite`（默认开启）：支持 SQLite 数据库。
- `mysql`：支持 MySQL 数据库。

```toml
[dependencies]
# 仅启用 MySQL 支持
uorm = { version = "0.4.1", default-features = false, features = ["mysql"] }
```

## 快速开始

### 1) 注册数据库驱动

通过 `UORM` 全局单例注册驱动。`SqliteDriver` 和 `MysqlDriver` 均采用 Builder 模式。

```rust
use uorm::driver_manager::UORM;
use uorm::udbc::sqlite::pool::SqliteDriver;

#[tokio::main]
async fn main() -> Result<(), uorm::error::DbError> {
    // 创建驱动并指定名称（默认为 "default"）
    let driver = SqliteDriver::new("sqlite::memory:")
        .build()?;
    
    // 注册到全局管理器
    UORM.register(driver)?;

    Ok(())
}
```

### 2) 加载 Mapper XML

`uorm` 提供两种方式加载 XML 资源：

**方式一：编译期内嵌（推荐）**
使用 `mapper_assets!` 宏，在编译时将 XML 文件内容嵌入二进制中，程序启动时自动注册。

```rust
use uorm::mapper_assets;

// 自动扫描路径下的所有 XML 文件并内嵌
mapper_assets!["src/resources/**/*.xml"];
```

**方式二：运行时加载**

在程序启动后手动扫描文件系统加载 XML。

```rust
use uorm::driver_manager::UORM;

fn init_mappers() -> Result<(), uorm::error::DbError> {
    // 运行时扫描并解析 XML
    UORM.assets("src/resources/**/*.xml")?;
    Ok(())
}
```

### 3) 执行 Mapper 调用

```rust
use serde::{Deserialize, Serialize};
use uorm::driver_manager::UORM;

#[derive(Debug, Deserialize)]
struct User {
    id: i64,
    name: String,
    age: i64,
}

#[derive(Serialize)]
struct IdArg {
    id: i64,
}

pub async fn get_user_by_id(user_id: i64) -> Result<User, uorm::error::DbError> {
    let mapper = UORM.mapper().expect("Driver not found");
    
    // execute 会根据 XML 定义的标签（select/insert/update/delete）自动执行。
    // 对于 select，如果结果只有一行且返回类型是结构体而非 Vec，会自动解包（Unwrap）。
    mapper.execute("user.get_by_id", &IdArg { id: user_id }).await
}
```

## SQL 过程宏 (`#[sql]`)

使用 `#[sql]` 宏可以像定义 DAO 接口一样操作数据库，代码更加优雅。

```rust
use serde::{Deserialize, Serialize};
use uorm::{exec, sql};

#[derive(Debug, Serialize, Deserialize)]
struct User {
    id: i64,
    name: String,
}

#[sql("user")] // 指定 XML 的 namespace
struct UserDao;

impl UserDao {
    #[sql("get_by_id")] // 对应 user.get_by_id
    pub async fn get(id: i64) -> Result<User, uorm::error::DbError> {
        // exec!() 是由 #[sql] 宏在函数内部注入的局部宏
        // 它会自动捕获函数参数、namespace 和 id 并执行调用
        exec!() 
    }

    #[sql(id = "list_all", database = "other_db")] // 可指定特定的数据库名称
    pub async fn list_all() -> Result<Vec<User>, uorm::error::DbError> {
        exec!()
    }
}
```

## 直接执行 SQL (`Session`)

如果不想使用 XML，也可以通过 `Session` 直接执行带有命名参数的 SQL。`uorm` 内部集成了轻量级模板引擎。

```rust
use serde::Serialize;
use uorm::driver_manager::UORM;

#[derive(Serialize)]
struct UserParam<'a> {
    name: &'a str,
    age: i32,
}

pub async fn add_user() -> Result<u64, uorm::error::DbError> {
    let session = UORM.session().expect("Default driver not found");

    // 支持 #{field} 语法绑定参数，内部会自动处理 SQL 注入防护
    let affected = session.execute(
        "INSERT INTO users(name, age) VALUES (#{name}, #{age})",
        &UserParam { name: "Alice", age: 18 }
    ).await?;

    Ok(affected)
}
```

## 事务管理

### 自动事务宏 (`#[transaction]`)

使用 `#[transaction]` 宏可以极大地简化事务代码。该宏会自动在函数开头注入 `session.begin()`，并根据返回值自动执行 `commit()` 或 `rollback()`。

```rust
use uorm::executor::session::Session;
use uorm::error::DbError;

#[uorm::transaction]
async fn transfer_data(session: &Session, data: MyData) -> Result<(), DbError> {
    // 宏会自动注入事务控制逻辑
    
    session.execute("INSERT ...", &data).await?;
    session.execute("UPDATE ...", &data).await?;
    
    Ok(())
}

// 如果参数名不是 session，可以显式指定：
#[uorm::transaction("s")]
async fn custom_session_name(s: &Session) -> Result<(), DbError> {
    Ok(())
}
```

### 手动管理事务

`uorm` 使用线程局部存储（Thread Local Storage）管理事务上下文，确保在同一线程内的操作共享同一个事务连接。

```rust
async fn manual_transaction() -> Result<(), uorm::error::DbError> {
    let session = UORM.session().expect("Default driver not found");
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
  <!-- 当 useGeneratedKeys 为 true 时，execute 将返回最后插入的 ID -->
  <insert id="insert_user" useGeneratedKeys="true" keyColumn="id">
    INSERT INTO users(name, age) VALUES (#{name}, #{age})
  </insert>
</mapper>
```

## 高级配置

### 连接池与超时

```rust
use uorm::udbc::ConnectionOptions;

fn configure_driver() -> Result<(), uorm::error::DbError> {
    let options = ConnectionOptions {
        max_open_conns: 20,
        max_idle_conns: 5,
        max_lifetime: 3600,
        timeout: 5, // 连接获取超时（秒）
    };

    let driver = MysqlDriver::new("mysql://user:pass@localhost/db")
        .options(options)
        .build()?;
    
    Ok(())
}
```

### SQLite 特殊说明

- **并发性**：SQLite 驱动默认开启了 `WAL` 模式（Write-Ahead Logging）和 `foreign_keys` 支持，显著提升并发读写性能。
- **内存数据库**：使用 `sqlite::memory:` 或 `sqlite://:memory:`。注意：默认配置下每次 `acquire()` 均会创建全新的内存库。若需共享，请使用 `cache=shared`。

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
