use std::sync::Arc;
use uorm::Param;
use uorm::executor::session::Session;
use uorm::udbc::driver::Driver;
use uorm::udbc::sqlite::pool::SqliteDriver;

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

#[tokio::test(flavor = "current_thread")]
async fn test_transaction_commit() {
    let db_name = "tx_commit";
    let url = format!("sqlite:file:{}?mode=memory&cache=shared", db_name);
    let driver = SqliteDriver::new(url).name(db_name).build().unwrap();
    let driver = Arc::new(driver);

    // 保持一个连接以确保内存数据库持续存在
    let _keep_alive = driver.acquire().await.unwrap();

    // 创建表
    let mut conn = driver.acquire().await.unwrap();
    conn.execute(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)",
        &[],
    )
    .await
    .unwrap();
    drop(conn);

    let session = Session::new(driver.clone());

    uorm::executor::session::with_tx_context(|| async {
        session.begin().await.unwrap();

        let sql = "INSERT INTO users (name, age) VALUES (#{name}, #{age})";
        let user = NewUser {
            name: "Alice".to_string(),
            age: 30,
        };
        session.execute(sql, &user).await.unwrap();

        session.commit().await.unwrap();
    })
    .await;

    // 校验数据存在
    let count_sql = "SELECT * FROM users WHERE name = 'Alice'";
    let rows: Vec<User> = session.query(count_sql, &()).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Alice");
}

#[tokio::test(flavor = "current_thread")]
async fn test_transaction_rollback() {
    let db_name = "tx_rollback";
    let url = format!("sqlite:file:{}?mode=memory&cache=shared", db_name);
    let driver = SqliteDriver::new(url).name(db_name).build().unwrap();
    let driver = Arc::new(driver);

    // 保持一个连接以确保内存数据库持续存在
    let _keep_alive = driver.acquire().await.unwrap();

    // 创建表
    let mut conn = driver.acquire().await.unwrap();
    conn.execute(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)",
        &[],
    )
    .await
    .unwrap();
    drop(conn);

    let session = Session::new(driver.clone());

    uorm::executor::session::with_tx_context(|| async {
        session.begin().await.unwrap();

        let sql = "INSERT INTO users (name, age) VALUES (#{name}, #{age})";
        let user = NewUser {
            name: "Bob".to_string(),
            age: 25,
        };
        session.execute(sql, &user).await.unwrap();

        session.rollback().await.unwrap();
    })
    .await;

    // 校验数据不存在
    let select_sql = "SELECT * FROM users WHERE name = 'Bob'";
    let rows: Vec<User> = session.query(select_sql, &()).await.unwrap();
    assert_eq!(rows.len(), 0);
}
