use std::sync::Once;
use uorm::Param;
use uorm::Result;
use uorm::driver_manager::U;
#[cfg(feature = "mysql")]
use uorm::udbc::mysql::pool::MysqlDriver;
#[cfg(feature = "sqlite")]
use uorm::udbc::sqlite::pool::SqliteDriver;
use uorm::{mapper_assets, sql};

#[derive(Debug, Param)]
struct User {
    id: Option<i64>,
    name: Option<String>,
    age: Option<i32>,
    status: Option<String>,
    create_time: Option<String>,
}

#[derive(Param)]
struct InsertParams {
    name: String,
    age: i32,
}

#[derive(Debug, Param)]
struct AgeStats {
    age_sum: Option<i64>,
}

#[derive(Debug, Param)]
struct AgeStatsByName {
    name: Option<String>,
    age_sum: Option<i64>,
}

#[sql("user")]
struct UserDao;

impl UserDao {
    #[sql("sum_age")]
    pub async fn sum_age() -> Result<AgeStats> {
        exec!()
    }

    #[sql("sum_age_group_by_name")]
    pub async fn sum_age_group_by_name() -> Result<Vec<AgeStatsByName>> {
        exec!()
    }
    #[sql("insert")]
    pub async fn insert_struct(params: InsertParams) -> Result<i64> {
        exec!()
    }

    #[sql("insert")]
    pub async fn insert_map(
        params: std::collections::HashMap<String, String>,
    ) -> uorm::Result<i64> {
        exec!()
    }

    #[sql("insert")]
    pub async fn insert(name: String, age: i32) -> Result<i64> {
        exec!()
    }

    #[sql("insert")]
    pub async fn insert_borrowed(name: &str, age: i32) -> Result<i64> {
        exec!()
    }

    #[sql("get_by_id")]
    pub async fn get_by_id(id: i64) -> Result<Vec<User>> {
        exec!()
    }

    #[sql("get_by_id")]
    pub async fn get_one_by_id(id: i64) -> Result<User> {
        exec!()
    }

    #[sql("get_by_id")]
    pub async fn get_option_by_id(id: i64) -> Result<Option<User>> {
        exec!()
    }

    #[sql("list_all")]
    pub async fn list_all() -> Result<Vec<User>> {
        exec!()
    }

    #[sql("update_age")]
    pub async fn update_age(id: i64, age: i32) -> Result<u64> {
        exec!()
    }

    #[sql(id = "get_by_id", namespace = "user")]
    pub async fn get_by_id_named(id: i64) -> Result<Vec<User>> {
        exec!()
    }

    #[sql("insert_with_date")]
    pub async fn insert_with_date(name: String, age: i32, create_time: String) -> Result<i64> {
        exec!()
    }

    #[sql("list_all_full")]
    pub async fn list_all_full() -> Result<Vec<User>> {
        exec!()
    }
}

static INIT: Once = Once::new();

// Use mapper_assets to load the XML at compile time
mapper_assets!["tests/resources/mapper"];

async fn setup_db() -> Box<dyn uorm::udbc::connection::Connection> {
    // Only init logger once
    INIT.call_once(|| {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    });

    // Re-register driver for every test to ensure pool is bound to current runtime
    #[cfg(feature = "sqlite")]
    {
        let url = "sqlite:file:macro_test?mode=memory&cache=shared";
        let driver = SqliteDriver::new(url).build().unwrap();
        U.register(driver).unwrap();
    }

    #[cfg(feature = "mysql")]
    {
        let url = "mysql://username:password@192.168.1.118:2881/test";
        println!("Connecting to MySQL URL: {}", url);
        let driver = MysqlDriver::new(url).build().unwrap();
        U.register(driver).unwrap();
    }

    let mapper = U.mapper().unwrap();
    println!("Acquiring connection...");
    let mut conn = mapper.pool.acquire().await.unwrap();
    println!("Connection acquired!");
    
    #[cfg(feature = "sqlite")]
    let create_sql = "CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT,
        age INTEGER,
        status TEXT DEFAULT 'active',
        create_time DATETIME DEFAULT CURRENT_TIMESTAMP
    )";

    #[cfg(feature = "mysql")]
    let create_sql = "CREATE TABLE IF NOT EXISTS users (
        id BIGINT PRIMARY KEY AUTO_INCREMENT,
        name VARCHAR(255),
        age INT,
        status VARCHAR(50) DEFAULT 'active',
        create_time DATETIME DEFAULT CURRENT_TIMESTAMP
    )";

    // Drop table to ensure clean state for each test (since we use AUTO_INCREMENT)
    conn.execute("DROP TABLE IF EXISTS users", &[]).await.unwrap();
    conn.execute(create_sql, &[]).await.unwrap();
    conn
}

#[tokio::test]
async fn test_user_dao_macros() {
    let _conn = setup_db().await;

    // 1. Test insert
    let affected = UserDao::insert("Alice".to_string(), 20).await.unwrap();
    assert!(affected >= 0);

    // 1.1 Test insert_borrowed
    let affected = UserDao::insert_borrowed("AliceBorrowed", 21).await.unwrap();
    assert!(affected >= 0);

    // 1.2 Test insert_struct
    let params = InsertParams {
        name: "AliceStruct".to_string(),
        age: 22,
    };
    let _ = UserDao::insert_struct(params).await.unwrap();

    // Verify insertion
    let users = UserDao::list_all().await.unwrap();
    let user = users
        .iter()
        .find(|u| u.name.as_deref() == Some("AliceStruct"))
        .expect("AliceStruct not found");
    assert_eq!(user.age, Some(22));

    // 1.3 Test insert_map
    let mut map = std::collections::HashMap::new();
    map.insert("name".to_string(), "AliceMap".to_string());
    map.insert("age".to_string(), "23".to_string());
    let _ = UserDao::insert_map(map).await.unwrap();

    // Verify insertion
    let users = UserDao::list_all().await.unwrap();
    let user = users
        .iter()
        .find(|u| u.name.as_deref() == Some("AliceMap"))
        .expect("AliceMap not found");
    // SQLite might return 23 as i32 even if inserted as string, due to column affinity.
    // But let's check.
    assert_eq!(user.age, Some(23));

    // 2. Test get_by_id
    let users = UserDao::get_by_id(1).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name.as_deref(), Some("Alice"));
    assert_eq!(users[0].age, Some(20));

    let user = UserDao::get_one_by_id(1).await.unwrap();
    assert_eq!(user.name.as_deref(), Some("Alice"));
    assert_eq!(user.age, Some(20));

    let missing = UserDao::get_option_by_id(999).await.unwrap();
    assert!(missing.is_none());

    let missing_err = UserDao::get_one_by_id(999).await.unwrap_err();
    assert!(matches!(missing_err, uorm::error::DbError::DbError(_)));

    // 3. Test list_all
    UserDao::insert("Bob".to_string(), 30).await.unwrap();
    let users = UserDao::list_all().await.unwrap();
    assert!(users.len() >= 2);

    // 4. Test update
    let alice_id = users
        .iter()
        .find(|u| u.name.as_deref() == Some("Alice"))
        .unwrap()
        .id
        .unwrap();
    let affected = UserDao::update_age(alice_id, 21).await.unwrap();
    assert_eq!(affected, 1);

    // 5. Verify update
    let updated_users = UserDao::get_by_id_named(alice_id).await.unwrap();
    assert_eq!(updated_users[0].age, Some(21));
}

#[tokio::test]
async fn test_date_formats() {
    let _conn = setup_db().await;

    macro_rules! test_date_format {
        ($name:expr, $date:expr) => {
            let _ = UserDao::insert_with_date($name.to_string(), 25, $date.to_string())
                .await
                .unwrap();
            let users = UserDao::list_all_full().await.unwrap();
            let user = users
                .iter()
                .find(|u| u.name.as_deref() == Some($name))
                .unwrap();
            
            let db_val = user.create_time.as_deref().unwrap_or("").replace("T", " ");
            let expected = $date.replace("T", " ");
            
            // Handle incomplete date string (e.g. "2023-10-01" vs "2023-10-01 00:00:00")
            if !db_val.contains(&expected) && !expected.contains(&db_val) {
                 assert_eq!(db_val, expected);
            }
        };
    }

    test_date_format!("DateUser1", "2023-10-01 10:00:00");
    test_date_format!("DateUser2", "2023-10-01T10:00:00");
    test_date_format!("DateUser3", "2023-10-01");
}

#[tokio::test]
async fn test_sum_age() {
    let mut conn = setup_db().await;
    // Clean up to ensure deterministic result
    conn.execute("DELETE FROM users", &[]).await.unwrap();

    // Insert some users
    UserDao::insert("User1".to_string(), 10).await.unwrap();
    UserDao::insert("User2".to_string(), 20).await.unwrap();
    UserDao::insert("User3".to_string(), 30).await.unwrap();

    let stats = UserDao::sum_age().await.unwrap();
    assert_eq!(stats.age_sum, Some(60));
}

#[tokio::test]
async fn test_sum_age_group_by_name() {
    let mut conn = setup_db().await;
    // Clean up
    conn.execute("DELETE FROM users", &[]).await.unwrap();

    // Insert users:
    // Group A: 10 + 20 = 30
    // Group B: 30
    UserDao::insert("GroupA".to_string(), 10).await.unwrap();
    UserDao::insert("GroupA".to_string(), 20).await.unwrap();
    UserDao::insert("GroupB".to_string(), 30).await.unwrap();

    let stats_list = UserDao::sum_age_group_by_name().await.unwrap();
    
    assert_eq!(stats_list.len(), 2);
    
    // GroupA
    assert_eq!(stats_list[0].name.as_deref(), Some("GroupA"));
    assert_eq!(stats_list[0].age_sum, Some(30));

    // GroupB
    assert_eq!(stats_list[1].name.as_deref(), Some("GroupB"));
    assert_eq!(stats_list[1].age_sum, Some(30));
}
