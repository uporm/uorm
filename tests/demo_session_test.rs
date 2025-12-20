#![cfg(feature = "sqlite")]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use uorm::driver_manager::UORM;
use uorm::udbc::sqlite::pool::SqliteDriver;

static TEST_SEQ: AtomicUsize = AtomicUsize::new(0);

fn unique_id(prefix: &str) -> String {
    let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}_{seq}_{nanos}")
}

fn temp_sqlite_file(prefix: &str) -> (String, PathBuf) {
    let file_name = format!("{}.db", unique_id(prefix));
    let path = std::env::temp_dir().join(file_name);
    let url = format!("sqlite:{}", path.display());
    (url, path)
}

async fn setup_sqlite(prefix: &str) -> (String, PathBuf) {
    let db_name = unique_id(prefix);
    let (url, path) = temp_sqlite_file(&db_name);
    let driver = SqliteDriver::new(url)
        .name(db_name.clone())
        .build()
        .unwrap();
    UORM.register(driver).unwrap();
    (db_name, path)
}

async fn create_users_table(db_name: &str) {
    #[derive(Serialize)]
    struct NoArgs {}

    let session = UORM.session(db_name).unwrap();
    session
        .execute(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, age INTEGER NOT NULL)",
            &NoArgs {},
        )
        .await
        .unwrap();
}

#[derive(Debug, Serialize, Deserialize)]
struct User {
    id: i64,
    name: String,
    age: i64,
}

#[tokio::test]
async fn demo_session_basic_crud() {
    #[derive(Serialize)]
    struct NewUser<'a> {
        name: &'a str,
        age: i64,
    }

    #[derive(Serialize)]
    struct NoArgs {}

    #[derive(Serialize)]
    struct NameArg<'a> {
        name: &'a str,
    }

    let (db_name, db_path) = setup_sqlite("demo_session_basic_crud").await;

    create_users_table(&db_name).await;

    let session = UORM.session(&db_name).unwrap();

    session
        .execute(
            "INSERT INTO users(name, age) VALUES (#{name}, #{age})",
            &NewUser {
                name: "alice",
                age: 18,
            },
        )
        .await
        .unwrap();

    session
        .execute(
            "INSERT INTO users(name, age) VALUES (#{name}, #{age})",
            &NewUser {
                name: "bob",
                age: 20,
            },
        )
        .await
        .unwrap();

    let mut rows: Vec<User> = session
        .query("SELECT id, name, age FROM users ORDER BY id", &NoArgs {})
        .await
        .unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].name, "alice");
    assert_eq!(rows[1].name, "bob");

    session
        .execute(
            "UPDATE users SET age = #{age} WHERE name = #{name}",
            &NewUser {
                name: "alice",
                age: 19,
            },
        )
        .await
        .unwrap();

    let alice: Vec<User> = session
        .query(
            "SELECT id, name, age FROM users WHERE name = #{name}",
            &NameArg { name: "alice" },
        )
        .await
        .unwrap();
    assert_eq!(alice.len(), 1);
    assert_eq!(alice[0].age, 19);

    session
        .execute(
            "DELETE FROM users WHERE name = #{name}",
            &NameArg { name: "bob" },
        )
        .await
        .unwrap();

    rows = session
        .query("SELECT id, name, age FROM users ORDER BY id", &NoArgs {})
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "alice");

    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
async fn demo_transaction_with_last_insert_id() {
    #[derive(Serialize)]
    struct NewUser<'a> {
        name: &'a str,
        age: i64,
    }

    #[derive(Serialize)]
    struct IdArg {
        id: i64,
    }

    let (db_name, db_path) = setup_sqlite("demo_transaction_with_last_insert_id").await;
    create_users_table(&db_name).await;

    let session = UORM.session(&db_name).unwrap();

    let mut tx = session.begin().await.unwrap();
    tx.execute(
        "INSERT INTO users(name, age) VALUES (#{name}, #{age})",
        &NewUser {
            name: "carol",
            age: 22,
        },
    )
    .await
    .unwrap();

    let id = tx.last_insert_id().await.unwrap() as i64;
    tx.commit().await.unwrap();

    let got: Vec<User> = session
        .query(
            "SELECT id, name, age FROM users WHERE id = #{id}",
            &IdArg { id },
        )
        .await
        .unwrap();

    assert_eq!(got.len(), 1);
    assert_eq!(got[0].name, "carol");
    assert_eq!(got[0].age, 22);

    let _ = std::fs::remove_file(db_path);
}
