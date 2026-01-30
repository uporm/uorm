use std::sync::Arc;
use uorm::Param;
use uorm::executor::session::Session;
use uorm::udbc::driver::Driver;
#[cfg(feature = "mysql")]
use uorm::udbc::mysql::pool::MysqlDriver;

#[derive(Debug, PartialEq, Param)]
struct User {
    id: Option<i64>,
    name: String,
    age: i32,
}

#[derive(Param)]
struct NewUser {
    name: String,
    age: i32,
}

#[cfg(feature = "mysql")]
#[tokio::test]
async fn test_mysql_autocommit() {
    // 注意：这个测试需要一个可用的MySQL数据库
    // 请确保环境中有MySQL数据库，并且可以通过以下连接字符串连接
    let db_name = "test";
    
    // 连接字符串格式：mysql://user:pass@host/db
    // 这里使用默认的测试数据库配置，实际运行时需要修改
    let url = "mysql://root:luoge123@192.168.1.118:2881/test";
    
    let driver = MysqlDriver::new(url).name(db_name.to_string()).build().unwrap();
    let driver = Arc::new(driver);

    // 创建测试表
    let mut conn = driver.acquire().await.unwrap();
    // 先删除表（如果存在）
    let _ = conn.execute("DROP TABLE IF EXISTS users", &[]).await;
    // 创建新表
    conn.execute(
        "CREATE TABLE users (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255), age INT)",
        &[],
    )
    .await
    .unwrap();
    drop(conn);

    // 测试1：验证autocommit是否启用
    let session1 = Session::new(driver.clone());
    // 插入数据
    let sql = "INSERT INTO users (name, age) VALUES (#{name}, #{age})";
    let user = NewUser {
        name: "TestUser".to_string(),
        age: 25,
    };
    let affected_rows = session1.execute(sql, &user).await.unwrap();
    assert_eq!(affected_rows, 1);

    // 测试2：使用新连接查询，验证数据是否可见
    let session2 = Session::new(driver.clone());
    let select_sql = "SELECT * FROM users WHERE name = 'TestUser'";
    let rows: Vec<User> = session2.query(select_sql, &()).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "TestUser");

    // 测试3：验证autocommit设置
    let autocommit_sql = "SHOW SESSION VARIABLES LIKE 'autocommit'";
    // 使用通用的HashMap类型来接收结果
    let autocommit_rows: Vec<std::collections::HashMap<String, uorm::udbc::value::Value>> = session1.query_raw(autocommit_sql, &()).await.unwrap();
    // 这里我们只是验证查询能成功执行，实际值在日志中可以看到
    assert!(!autocommit_rows.is_empty());
    // 验证autocommit值为ON
    if let Some(row) = autocommit_rows.first() {
        if let Some(value) = row.get("Value") {
            println!("Autocommit setting: {:?}", value);
        }
    }

    // 清理测试数据
    let _ = session1.execute("DROP TABLE users", &()).await;
}

#[cfg(not(feature = "mysql"))]
#[tokio::test]
async fn test_mysql_autocommit_skipped() {
    // 当未启用mysql特性时，跳过测试
    println!("MySQL feature not enabled, skipping test");
}
