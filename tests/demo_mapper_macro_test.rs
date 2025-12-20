#![cfg(feature = "sqlite")]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use uorm::driver_manager::UORM;
use uorm::mapper_loader;
use uorm::udbc::sqlite::pool::SqliteDriver;
use uorm::{sql_delete, sql_get, sql_insert, sql_list, sql_namespace, sql_update};

static TEST_SEQ: AtomicUsize = AtomicUsize::new(0);
static TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

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

async fn register_sqlite(db_name: &str, prefix: &str) -> PathBuf {
    let (url, path) = temp_sqlite_file(prefix);
    let driver = SqliteDriver::new(url)
        .name(db_name.to_string())
        .build()
        .unwrap();
    UORM.register(driver).unwrap();
    path
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct User {
    id: i64,
    name: String,
    age: i64,
}

#[derive(Debug, Serialize)]
struct NewUser {
    id: i64,
    name: String,
    age: i64,
}

#[sql_namespace("macro_user")]
struct UserDao;

impl UserDao {
    #[sql_insert(id = "insert_user", db_name = "demo_mapper_macro")]
    pub async fn create(user: NewUser) -> Result<i64, uorm::error::DbError> {
        exec!()
    }

    #[sql_get(id = "get_by_id", db_name = "demo_mapper_macro")]
    pub async fn get(id: i64) -> Result<User, uorm::error::DbError> {
        exec!()
    }

    #[sql_list(id = "list_all", db_name = "demo_mapper_macro")]
    pub async fn list_all(args: ()) -> Result<Vec<User>, uorm::error::DbError> {
        exec!()
    }

    #[sql_list(id = "list_by_min_age", db_name = "demo_mapper_macro")]
    pub async fn list_by_min_age(min_age: i64) -> Result<Vec<User>, uorm::error::DbError> {
        exec!()
    }

    #[sql_update(id = "update_age", db_name = "demo_mapper_macro")]
    pub async fn update_age(id: i64, age: i64) -> Result<u64, uorm::error::DbError> {
        exec!()
    }

    #[sql_delete(id = "delete_by_id", db_name = "demo_mapper_macro")]
    pub async fn delete(id: i64) -> Result<u64, uorm::error::DbError> {
        exec!()
    }

    #[sql_list(id = "list_by_ids", db_name = "demo_mapper_macro")]
    pub async fn list_by_ids(ids: Vec<i64>) -> Result<Vec<User>, uorm::error::DbError> {
        exec!()
    }

    #[sql_insert(id = "insert", db_name = "default")]
    pub async fn insert_auto_id(name: String, age: i64) -> Result<i64, uorm::error::DbError> {
        exec!()
    }
}

#[tokio::test]
async fn demo_mapper_macro_basic_crud() {
    let _guard = TEST_LOCK.lock().await;

    mapper_loader::clear_mappers();
    UORM.assets("tests/resources/mapper/macro_user.xml")
        .unwrap();

    let db_path = register_sqlite("demo_mapper_macro", "demo_mapper_macro_basic_crud").await;
    create_users_table("demo_mapper_macro").await;

    let affected = UserDao::create(NewUser {
        id: 1,
        name: "alice".to_string(),
        age: 18,
    })
    .await
    .unwrap();
    assert_eq!(affected, 1);

    let affected = UserDao::create(NewUser {
        id: 2,
        name: "bob".to_string(),
        age: 21,
    })
    .await
    .unwrap();
    assert_eq!(affected, 1);

    let alice = UserDao::get(1).await.unwrap();
    assert_eq!(
        alice,
        User {
            id: 1,
            name: "alice".to_string(),
            age: 18
        }
    );

    let older = UserDao::list_by_min_age(20).await.unwrap();
    assert_eq!(older.len(), 1);
    assert_eq!(older[0].name, "bob");

    let affected = UserDao::update_age(1, 19).await.unwrap();
    assert_eq!(affected, 1);

    let alice = UserDao::get(1).await.unwrap();
    assert_eq!(alice.age, 19);

    let affected = UserDao::delete(2).await.unwrap();
    assert_eq!(affected, 1);

    let all = UserDao::list_all(()).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id, 1);

    // 测试 list_by_ids
    // 先重新插入一些数据用于测试
    let affected = UserDao::create(NewUser {
        id: 3,
        name: "carol".to_string(),
        age: 22,
    })
    .await
    .unwrap();
    assert_eq!(affected, 1);

    let affected = UserDao::create(NewUser {
        id: 4,
        name: "dave".to_string(),
        age: 25,
    })
    .await
    .unwrap();
    assert_eq!(affected, 1);

    let got = UserDao::list_by_ids(vec![1, 3, 4]).await.unwrap();
    assert_eq!(got.len(), 3);
    let ids: Vec<i64> = got.into_iter().map(|u| u.id).collect();
    assert_eq!(ids, vec![1, 3, 4]);

    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
async fn demo_mapper_macro_auto_id_insert() {
    let _guard = TEST_LOCK.lock().await;

    mapper_loader::clear_mappers();
    UORM.assets("tests/resources/mapper/macro_user.xml")
        .unwrap();

    // 为 default 数据库创建临时数据库
    let db_path = register_sqlite("default", "demo_mapper_macro_auto_id_insert").await;
    create_users_table("default").await;

    // 测试自增 ID 插入
    let affected = UserDao::insert_auto_id("alice".to_string(), 18)
        .await
        .unwrap();
    assert_eq!(affected, 1);

    let affected = UserDao::insert_auto_id("bob".to_string(), 20)
        .await
        .unwrap();
    assert_eq!(affected, 1);

    // 使用 list_all 验证插入的数据
    // 注意：list_all 使用的是 demo_mapper_macro 数据库，但我们需要为 default 数据库创建一个新的 UserDao
    // 或者我们可以直接使用 mapper 来查询
    let mapper = UORM.mapper("default").unwrap();
    let all: Vec<User> = mapper.list("macro_user.list_all", &()).await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].name, "alice");
    assert_eq!(all[1].name, "bob");

    let _ = std::fs::remove_file(db_path);
}
