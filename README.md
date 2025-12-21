# uorm

[![Crates.io](https://img.shields.io/crates/v/uorm)](https://crates.io/crates/uorm)
[![Documentation](https://docs.rs/uorm/badge.svg)](https://docs.rs/uorm)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Build Status](https://github.com/uporm/uorm/actions/workflows/ci.yml/badge.svg)](https://github.com/uporm/uorm/actions)

Rust 下的轻量级 ORM 框架，借鉴 Java MyBatis 的设计理念，强调 SQL 与业务逻辑分离：用 XML 管理 SQL，通过 `sql_id` 调用并把结果自动反序列化到 Rust 类型；同时支持直接执行原生 SQL，兼容 `async/await`。

## 特性

- 🚀 **MyBatis 风格**：熟悉的 XML Mapper 语法，`namespace.id` 作为 SQL 标识
- 📦 **类型安全**：利用 Rust 强大的类型系统，编译时检查 SQL 参数和结果类型
- ⚡ **异步优先**：原生支持 `async/await`，基于 `tokio` 运行时
- 🔧 **灵活配置**：支持多数据源、连接池、事务管理
- 🎯 **动态 SQL**：支持 `<if>`、`<foreach>` 等动态 SQL 标签
- 🛠️ **过程宏**：编译期内嵌 XML、自动生成 DAO 方法
- 🗄️ **多数据库**：支持 SQLite、MySQL，易于扩展其他数据库
- 📝 **详细日志**：集成 `log` crate，便于调试和监控

## 目录

- [快速开始](#快速开始sqlite--xml-mapper)
- [功能概览](#功能概览)
- [安装](#安装)
- [直接执行 SQL](#直接执行-sqlsession)
- [事务](#事务)
- [SQL 属性宏](#sql-属性宏sql_)
- [XML Mapper 格式](#xml-mapper-格式)
- [支持的数据库](#支持的数据库)
- [高级功能](#高级功能)
- [配置选项](#配置选项)
- [开发与测试](#开发与测试)
- [贡献指南](#贡献指南)
- [常见问题](#常见问题)
- [社区](#社区)
- [License](#license)

## 功能概览

- MyBatis 风格 XML Mapper：`namespace.id` 作为 SQL 唯一标识
- 原生 SQL 模板渲染：`#{field}` 绑定参数，支持 `<if>` / `<foreach>`
- `Session`：执行任意 SQL（`execute`/`query`/`last_insert_id`）
- `Mapper`：按 `sql_id` 调用 XML 中的 `select/insert/update/delete`
- 多数据源：通过 `DriverManager` 注册多个库（按 `db_name` 区分）
- 过程宏：
  - `mapper_assets![...]`：编译期内嵌 XML 并在启动时自动加载
  - `sql_namespace` + `sql_get/sql_list/sql_insert/sql_update/sql_delete`：把 `sql_id` 绑定到 DAO 方法里

## 安装

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
uorm = "0.2.0"
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
```

数据库特性开关：

- SQLite（默认开启）：无需额外配置
- MySQL：启用 `mysql` feature

```toml
[dependencies]
uorm = { version = "0.2.0", default-features = false, features = ["mysql"] }
```

## 快速开始（SQLite + XML Mapper）

### 1) 注册数据库驱动

```rust
use uorm::driver_manager::UORM;
use uorm::udbc::sqlite::pool::SqliteDriver;

#[tokio::main]
async fn main() -> Result<(), uorm::error::DbError> {
    let driver = SqliteDriver::new("sqlite::memory:")
        .name("default".to_string())
        .build()?;
    UORM.register(driver)?;

    Ok(())
}
```

### 2) 加载 Mapper XML

运行时从文件加载：

```rust
use uorm::driver_manager::UORM;

fn load_mapper_xml() -> Result<(), uorm::error::DbError> {
    UORM.assets("src/resources/**/*.xml")?;
    Ok(())
}
```

或编译期内嵌并在启动时自动加载（适合二进制发布）：

```rust
use uorm::mapper_assets;

mapper_assets!["src/resources/**/*.xml"];
```

### 3) 调用 Mapper

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

pub async fn get_user() -> Result<User, uorm::error::DbError> {
    let mapper = UORM.mapper("default").unwrap();
    mapper.get("user.get_by_id", &IdArg { id: 1 }).await
}
```

## 直接执行 SQL（Session）

```rust
use serde::Serialize;
use uorm::driver_manager::UORM;

#[derive(Serialize)]
struct NewUser<'a> {
    name: &'a str,
    age: i64,
}

pub async fn create_user() -> Result<i64, uorm::error::DbError> {
    let session = UORM.session("default").unwrap();

    session
        .execute(
            "INSERT INTO users(name, age) VALUES (#{name}, #{age})",
            &NewUser { name: "alice", age: 18 },
        )
        .await?;

    Ok(session.last_insert_id().await? as i64)
}
```

## 事务

```rust
use serde::Serialize;
use uorm::driver_manager::UORM;

#[derive(Serialize)]
struct NewUser<'a> {
    name: &'a str,
    age: i64,
}

pub async fn create_in_tx() -> Result<i64, uorm::error::DbError> {
    let session = UORM.session("default").unwrap();

    let mut tx = session.begin().await?;
    tx.execute(
        "INSERT INTO users(name, age) VALUES (#{name}, #{age})",
        &NewUser { name: "bob", age: 20 },
    )
    .await?;

    let id = tx.last_insert_id().await? as i64;
    tx.commit().await?;
    Ok(id)
}
```

## SQL 属性宏（sql_*）

把 `namespace/id/db_name` 绑定到方法上，方法体里用 `exec!()` 执行对应 `sql_id`：

```rust
use serde::{Deserialize, Serialize};
use uorm::{exec, sql_get, sql_insert, sql_list, sql_namespace, sql_update};

#[derive(Debug, Serialize, Deserialize)]
struct User {
    id: i64,
    name: String,
    age: i64,
}

#[sql_namespace("user")]
struct UserDao;

impl UserDao {
    #[sql_get(id = "get_by_id", database = "default")]
    pub async fn get(id: i64) -> Result<User, uorm::error::DbError> {
        exec!()
    }

    #[sql_list(id = "list_all", database = "default")]
    pub async fn list_all(args: ()) -> Result<Vec<User>, uorm::error::DbError> {
        exec!()
    }

    #[sql_insert(id = "insert_user", database = "default")]
    pub async fn insert(user: User) -> Result<i64, uorm::error::DbError> {
        exec!()
    }

    #[sql_update(id = "update_age", database = "default")]
    pub async fn update_age(id: i64, age: i64) -> Result<u64, uorm::error::DbError> {
        exec!()
    }
}
```

说明：

- `exec!()` 只能在 `sql_*` 属性宏标注的方法体内使用（宏会注入运行时调用逻辑）
- `database` 对应 `UORM.register(driver)` 时 driver 的 `name()`

## XML Mapper 格式

`sql_id` 由 `namespace.id` 组成，例如：`user.get_by_id`。

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE mapper PUBLIC "-//uporm.github.io//DTD Mapper 1//EN" "https://uporm.github.io/dtd/uorm-1-mapper.dtd">
<mapper namespace="user">
  <select id="get_by_id">
    SELECT id, name, age FROM users WHERE id = #{id}
  </select>

  <insert id="insert_user" useGeneratedKeys="true" keyColumn="id">
    INSERT INTO users(name, age) VALUES (#{name}, #{age})
  </insert>

  <select id="list_by_ids">
    SELECT id, name, age FROM users
    WHERE id IN
    <foreach item="id" collection="ids" open="(" separator="," close=")">
      #{id}
    </foreach>
  </select>

  <select id="list_by_min_age">
    SELECT id, name, age FROM users WHERE 1=1
    <if test="age != null">
      AND age &gt;= #{age}
    </if>
  </select>
</mapper>
```

多数据库类型选择：

- 通过 SQL 节点的 `databaseType="mysql|sqlite|..."` 指定适配的数据库
- 同一 `id` 可定义多个版本；当找不到匹配的 `databaseType` 时会回退到未指定 `databaseType` 的版本

## 支持的数据库

- SQLite：`uorm::udbc::sqlite::pool::SqliteDriver`（默认 feature）
- MySQL：`uorm::udbc::mysql::pool::MysqlDriver`（需要 `mysql` feature）

## 开发与测试

```bash
cargo test
```

## 高级功能

### 动态 SQL 构建

uorm 支持 MyBatis 风格的动态 SQL 标签：

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE mapper PUBLIC "-//uporm.github.io//DTD Mapper 1//EN" "https://uporm.github.io/dtd/uorm-1-mapper.dtd">
<mapper namespace="example">
  <select id="search_users">
    SELECT * FROM users WHERE 1=1
    <if test="name != null and name != ''">
      AND name LIKE CONCAT('%', #{name}, '%')
    </if>
    <if test="min_age != null">
      AND age >= #{min_age}
    </if>
    <if test="max_age != null">
      AND age &lt;= #{max_age}
    </if>
    <if test="ids != null and ids.size() > 0">
      AND id IN
      <foreach item="id" collection="ids" open="(" separator="," close=")">
        #{id}
      </foreach>
    </if>
    ORDER BY id
  </select>
</mapper>
```

### 多数据库支持

同一 SQL 可以针对不同数据库进行优化：

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE mapper PUBLIC "-//uporm.github.io//DTD Mapper 1//EN" "https://uporm.github.io/dtd/uorm-1-mapper.dtd">
<mapper namespace="user">
  <select id="get_user" databaseType="mysql">
    SELECT * FROM users WHERE id = #{id} LIMIT 1
  </select>

  <select id="get_user" databaseType="sqlite">
    SELECT * FROM users WHERE id = #{id} LIMIT 1
  </select>
</mapper>
```

### 连接池配置

```rust
use uorm::udbc::sqlite::pool::SqliteDriver;

fn configure_pool() -> Result<(), uorm::error::DbError> {
    let driver = SqliteDriver::new("sqlite::memory:")
        .name("default".to_string())
        .max_connections(10)  // 最大连接数
        .min_connections(2)   // 最小连接数
        .connection_timeout(std::time::Duration::from_secs(30))
        .idle_timeout(std::time::Duration::from_secs(300))
        .build()?;
    
    Ok(())
}
```

## 配置选项

### 日志配置

uorm 使用 `log` crate 进行日志记录。启用调试日志：

```rust
use env_logger;

fn main() {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Debug)
        .init();
    
    // ... 其他代码
}
```

### 性能优化

1. **使用编译期内嵌 XML**：对于生产环境，使用 `mapper_assets!` 宏将 XML 编译到二进制中，避免运行时文件 IO。
2. **合理使用连接池**：根据应用负载调整连接池大小。
3. **批量操作**：对于大量数据操作，考虑使用事务或批量插入。



## 贡献指南

欢迎贡献！请遵循以下步骤：

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 打开 Pull Request

### 开发环境设置

```bash
# 克隆项目
git clone https://github.com/uporm/uorm.git
cd uorm

# 运行测试
cargo test --all-features

# 运行特定测试
cargo test --test demo_session_test

# 构建文档
cargo doc --open
```

### 代码风格

- 遵循 Rust 官方代码风格
- 使用 `rustfmt` 格式化代码
- 使用 `clippy` 进行代码检查

## 常见问题

### Q: 如何处理数据库迁移？
A: uorm 专注于数据访问层，建议使用专门的迁移工具如 `diesel` 或 `sqlx` 进行数据库迁移。

### Q: 是否支持 PostgreSQL？
A: 目前支持 SQLite 和 MySQL，PostgreSQL 支持正在开发中。

### Q: 如何监控性能？
A: 可以通过启用调试日志来监控 SQL 执行时间，或集成第三方监控工具。

### Q: 是否支持异步流？
A: 目前不支持异步流，但可以通过分页查询处理大量数据。

## 社区

- **GitHub**: [https://github.com/uporm/uorm](https://github.com/uporm/uorm)
- **问题追踪**: [GitHub Issues](https://github.com/uporm/uorm/issues)
- **讨论区**: [GitHub Discussions](https://github.com/uporm/uorm/discussions)


## License

Apache-2.0
