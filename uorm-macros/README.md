# uorm-macros
[![Crates.io](https://img.shields.io/crates/v/uorm-macros)](https://crates.io/crates/uorm-macros)
[![Documentation](https://docs.rs/uorm-macros/badge.svg)](https://docs.rs/uorm-macros)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

`uorm-macros` 是 [uorm](https://github.com/uporm/uorm) ORM 框架的过程宏集合，提供编译时资源内嵌和 SQL 方法绑定功能，简化基于 XML Mapper 的数据库操作。

## 特性

- 🚀 **编译时资源内嵌**：使用 `mapper_assets!` 宏在编译时将 XML Mapper 文件内嵌到二进制中
- 🎯 **类型安全 DAO**：通过 `sql_namespace` 和 `sql_*` 属性宏生成类型安全的数据库访问方法
- ⚡ **零运行时开销**：宏展开在编译时完成，运行时无额外开销
- 🔧 **灵活配置**：支持自定义 SQL ID、数据库名称等参数
- 📝 **无缝集成**：与 uorm 框架完美集成，提供完整的 ORM 体验

## 安装

将以下依赖添加到你的 `Cargo.toml`：

```toml
[dependencies]
uorm = "0.2"
uorm-macros = "0.2"
```

## 快速开始

### 1. 编译时内嵌 XML Mapper 资源

使用 `mapper_assets!` 宏在编译时加载 XML Mapper 文件：

```rust
use uorm::mapper_assets;

// 在程序启动时自动加载所有匹配的 XML 文件
mapper_assets!("resources/**/*.xml");
```

这个宏会：
- 在编译时查找匹配的 XML 文件
- 使用 `include_str!` 将文件内容内嵌到二进制中
- 生成一个启动时自动执行的函数来注册这些资源

### 2. 使用 SQL 属性宏创建 DAO

```rust
use serde::{Deserialize, Serialize};
use uorm::{exec, sql_get, sql_insert, sql_list, sql_namespace, sql_update};

#[derive(Debug, Serialize, Deserialize)]
struct User {
    id: i64,
    name: String,
    age: i64,
}

// 定义命名空间
#[sql_namespace("user")]
struct UserDao;

impl UserDao {
    // 查询单个用户
    #[sql_get(id = "get_by_id", db_name = "default")]
    pub async fn get(id: i64) -> Result<User, uorm::error::DbError> {
        exec!()
    }

    // 查询用户列表
    #[sql_list(id = "list_all", db_name = "default")]
    pub async fn list_all(args: ()) -> Result<Vec<User>, uorm::error::DbError> {
        exec!()
    }

    // 插入用户
    #[sql_insert(id = "insert_user", db_name = "default")]
    pub async fn insert(user: User) -> Result<i64, uorm::error::DbError> {
        exec!()
    }

    // 更新用户年龄
    #[sql_update(id = "update_age", db_name = "default")]
    pub async fn update_age(id: i64, age: i64) -> Result<u64, uorm::error::DbError> {
        exec!()
    }

    // 删除用户
    #[sql_delete(id = "delete_user", db_name = "default")]
    pub async fn delete(id: i64) -> Result<u64, uorm::error::DbError> {
        exec!()
    }
}
```

## 宏详解

### `mapper_assets!`

**功能**：在编译时内嵌 XML Mapper 资源文件。

**参数**：
- `pattern`：Glob 模式字符串，用于匹配 XML 文件路径

**示例**：
```rust
// 加载单个文件
mapper_assets!("resources/user.xml");

// 加载目录下所有 XML 文件
mapper_assets!("resources/**/*.xml");

// 加载多个目录
mapper_assets!("resources/mappers/*.xml");
```

**工作原理**：
1. 在编译时使用 `glob` 模式查找匹配的文件
2. 使用 `include_str!` 将文件内容内嵌到二进制中
3. 生成一个 `#[uorm::ctor::ctor]` 修饰的函数，在程序启动时自动执行
4. 调用 `uorm::mapper_loader::load_assets()` 注册资源

### `sql_namespace`

**功能**：为 DAO 结构体定义 XML Mapper 的命名空间。

**参数**：
- `namespace`：XML Mapper 中定义的命名空间字符串

**示例**：
```rust
#[sql_namespace("user")]
struct UserDao;
```

**生成代码**：
- 为结构体添加 `NAMESPACE` 常量
- 例如：`pub const NAMESPACE: &'static str = "user";`

### `sql_get` / `sql_list` / `sql_insert` / `sql_update` / `sql_delete`

**功能**：将 SQL 操作绑定到异步方法上。

**参数**：
- `id`（可选）：XML Mapper 中的 SQL ID，默认为方法名
- `db_name`（可选）：数据库名称，默认为 "default"

**支持两种参数格式**：

1. **位置参数**：
```rust
#[sql_get("get_by_id")]
pub async fn get(id: i64) -> Result<User, uorm::error::DbError> {
    exec!()
}
```

2. **命名参数**：
```rust
#[sql_get(id = "get_by_id", db_name = "users_db")]
pub async fn get(id: i64) -> Result<User, uorm::error::DbError> {
    exec!()
}
```

**`exec!()` 宏**：
- 只能在 `sql_*` 属性宏标注的方法体内使用
- 宏会注入运行时调用逻辑，执行对应的 SQL 操作
- 自动处理参数序列化和结果反序列化

## 完整示例

### 项目结构
```
src/
├── main.rs
├── dao/
│   └── user_dao.rs
└── resources/
    └── user.xml
```

### XML Mapper 文件 (`resources/user.xml`)
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE mapper PUBLIC "-//uporm.github.io//DTD Mapper 1//EN" "https://uporm.github.io/dtd/uorm-1-mapper.dtd">
<mapper namespace="user">
  <select id="get_by_id">
    SELECT id, name, age FROM users WHERE id = #{id}
  </select>

  <select id="list_all">
    SELECT id, name, age FROM users
  </select>

  <insert id="insert_user" useGeneratedKeys="true" keyColumn="id">
    INSERT INTO users(name, age) VALUES (#{name}, #{age})
  </insert>

  <update id="update_age">
    UPDATE users SET age = #{age} WHERE id = #{id}
  </update>

  <delete id="delete_user">
    DELETE FROM users WHERE id = #{id}
  </delete>
</mapper>
```

### DAO 定义 (`src/dao/user_dao.rs`)
```rust
use serde::{Deserialize, Serialize};
use uorm::{exec, sql_delete, sql_get, sql_insert, sql_list, sql_namespace, sql_update};

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub age: i64,
}

#[sql_namespace("user")]
pub struct UserDao;

impl UserDao {
    #[sql_get("get_by_id")]
    pub async fn get_by_id(id: i64) -> Result<User, uorm::error::DbError> {
        exec!()
    }

    #[sql_list("list_all")]
    pub async fn list_all() -> Result<Vec<User>, uorm::error::DbError> {
        exec!()
    }

    #[sql_insert("insert_user")]
    pub async fn insert(name: String, age: i64) -> Result<i64, uorm::error::DbError> {
        exec!()
    }

    #[sql_update("update_age")]
    pub async fn update_age(id: i64, age: i64) -> Result<u64, uorm::error::DbError> {
        exec!()
    }

    #[sql_delete("delete_user")]
    pub async fn delete(id: i64) -> Result<u64, uorm::error::DbError> {
        exec!()
    }
}
```

### 主程序 (`src/main.rs`)
```rust
use uorm::mapper_assets;
use uorm::udbc::sqlite::SqliteDriver;
use uorm::driver_manager::UORM;

mod dao;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 内嵌 XML 资源
    mapper_assets!("resources/**/*.xml");

    // 2. 注册数据库驱动
    let driver = SqliteDriver::new("sqlite:test.db")?;
    UORM.register(driver)?;

    // 3. 使用 DAO 操作数据库
    let user_id = dao::UserDao::insert("Alice".to_string(), 25).await?;
    println!("Inserted user with id: {}", user_id);

    let user = dao::UserDao::get_by_id(user_id).await?;
    println!("Retrieved user: {:?}", user);

    let users = dao::UserDao::list_all().await?;
    println!("Total users: {}", users.len());

    Ok(())
}
```

## 高级用法

### 自定义参数处理

你可以在 `exec!()` 前后添加自定义逻辑：

```rust
#[sql_get("get_by_id")]
pub async fn get_with_logging(id: i64) -> Result<User, uorm::error::DbError> {
    println!("Fetching user with id: {}", id);
    let result = exec!();
    println!("Fetch completed");
    result
}
```

### 多数据库支持

```rust
#[sql_namespace("user")]
struct UserDao;

impl UserDao {
    // 使用默认数据库
    #[sql_get("get_by_id")]
    pub async fn get_default(id: i64) -> Result<User, uorm::error::DbError> {
        exec!()
    }

    // 使用特定数据库
    #[sql_get(id = "get_by_id", db_name = "replica_db")]
    pub async fn get_from_replica(id: i64) -> Result<User, uorm::error::DbError> {
        exec!()
    }
}
```

### 动态 SQL 参数

支持复杂的参数结构：

```rust
#[derive(Serialize)]
struct QueryParams {
    min_age: i64,
    max_age: i64,
    name_pattern: String,
}

#[sql_list("search_users")]
pub async fn search(params: QueryParams) -> Result<Vec<User>, uorm::error::DbError> {
    exec!()
}
```

对应的 XML：
```xml
<select id="search_users">
  SELECT id, name, age FROM users 
  WHERE age BETWEEN #{min_age} AND #{max_age}
    AND name LIKE #{name_pattern}
</select>
```

## 注意事项

1. **`exec!()` 宏限制**：只能在 `sql_*` 属性宏标注的方法体内使用
2. **异步方法**：所有生成的方法都是 `async fn`
3. **错误处理**：方法返回 `Result<T, uorm::error::DbError>`
4. **编译时检查**：SQL ID 和命名空间在编译时验证
5. **资源加载**：确保在调用 DAO 方法前已注册数据库驱动和加载 XML 资源

## 贡献

欢迎提交 Issue 和 Pull Request！请参考 [uorm 主项目](https://github.com/uporm/uorm) 的贡献指南。

## License

本项目基于 Apache License 2.0 开源。详见 [LICENSE](LICENSE) 文件。
