use uorm::udbc::sqlite::pool::SqliteDriver;
use serde::{Deserialize, Serialize};
use std::sync::Once;
use uorm::driver_manager::UORM;
use uorm::{sql, mapper_assets};

#[derive(Debug, Serialize, Deserialize)]
struct User {
    id: Option<i64>,
    name: Option<String>,
    age: Option<i32>,
    status: Option<String>,
    create_time: Option<String>,
}

#[sql("user")]
struct UserDao;

impl UserDao {
    #[sql("insert")]
    pub async fn insert(name: String, age: i32) -> Result<i64, uorm::error::DbError> {
        exec!()
    }

    #[sql("get_by_id")]
    pub async fn get_by_id(id: i64) -> Result<Vec<User>, uorm::error::DbError> {
        exec!()
    }

    #[sql("list_all")]
    pub async fn list_all() -> Result<Vec<User>, uorm::error::DbError> {
        exec!()
    }

    #[sql("update_age")]
    pub async fn update_age(id: i64, age: i32) -> Result<u64, uorm::error::DbError> {
        exec!()
    }

    #[sql(id = "get_by_id", namespace = "user")]
    pub async fn get_by_id_named(id: i64) -> Result<Vec<User>, uorm::error::DbError> {
        exec!()
    }
}

static INIT: Once = Once::new();

// Use mapper_assets to load the XML at compile time
mapper_assets!["tests/resources/mapper/user.xml"];

async fn setup_db() -> Box<dyn uorm::udbc::connection::Connection> {
    INIT.call_once(|| {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
        
        let url = "sqlite:file:macro_test?mode=memory&cache=shared";
        let driver = SqliteDriver::new(url).build().unwrap(); // Default name is "default"
        
        // Register the driver to UORM
        UORM.register(driver).unwrap();
    });

    let mapper = UORM.mapper().unwrap();
    let mut conn = mapper.pool.acquire().await.unwrap();
    conn.execute("CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT,
        age INTEGER,
        status TEXT DEFAULT 'active',
        create_time DATETIME DEFAULT CURRENT_TIMESTAMP
    )", &[]).await.unwrap();
    conn
}

#[tokio::test]
async fn test_user_dao_macros() {
    let _conn = setup_db().await;

    // 1. Test insert
    let affected = UserDao::insert("Alice".to_string(), 20).await.unwrap();
    assert!(affected >= 0);

    // 2. Test get_by_id
    let users = UserDao::get_by_id(1).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name.as_deref(), Some("Alice"));
    assert_eq!(users[0].age, Some(20));

    // 3. Test list_all
    UserDao::insert("Bob".to_string(), 30).await.unwrap();
    let users = UserDao::list_all().await.unwrap();
    assert!(users.len() >= 2);

    // 4. Test update
    let alice_id = users.iter().find(|u| u.name.as_deref() == Some("Alice")).unwrap().id.unwrap();
    let affected = UserDao::update_age(alice_id, 21).await.unwrap();
    assert_eq!(affected, 1);

    // 5. Verify update
    let updated_users = UserDao::get_by_id_named(alice_id).await.unwrap();
    assert_eq!(updated_users[0].age, Some(21));
}
