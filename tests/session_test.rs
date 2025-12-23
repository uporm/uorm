use uorm::udbc::sqlite::pool::SqliteDriver;
use uorm::udbc::connection::Connection;
use uorm::executor::session::Session;
use uorm::udbc::driver::Driver;
use serde::{Serialize, Deserialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct User {
    id: Option<i64>,
    name: String,
    age: i32,
}

#[derive(Serialize)]
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
    
    // Keep a connection open to ensure memory DB persists
    let _keep_alive = driver.acquire().await.unwrap();

    // Create table
    let mut conn = driver.acquire().await.unwrap();
    conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)", &[]).await.unwrap();
    drop(conn);

    let session = Session::new(driver.clone());

    // Begin transaction
    session.begin().await.unwrap();

    // Insert data
    let sql = "INSERT INTO users (name, age) VALUES (#{name}, #{age})";
    let user = NewUser { name: "Alice".to_string(), age: 30 };
    session.execute(sql, &user).await.unwrap();

    // Commit
    session.commit().await.unwrap();

    // Verify data exists
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
    
    // Keep a connection open to ensure memory DB persists
    let _keep_alive = driver.acquire().await.unwrap();
    
    // Create table
    let mut conn = driver.acquire().await.unwrap();
    conn.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)", &[]).await.unwrap();
    drop(conn);

    let session = Session::new(driver.clone());

    // Begin transaction
    session.begin().await.unwrap();

    // Insert data
    let sql = "INSERT INTO users (name, age) VALUES (#{name}, #{age})";
    let user = NewUser { name: "Bob".to_string(), age: 25 };
    session.execute(sql, &user).await.unwrap();

    // Rollback
    session.rollback().await.unwrap();

    // Verify data does NOT exist
    let select_sql = "SELECT * FROM users WHERE name = 'Bob'";
    let rows: Vec<User> = session.query(select_sql, &()).await.unwrap();
    assert_eq!(rows.len(), 0);
}
