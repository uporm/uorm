use std::sync::Once;
use uorm::Param;
use uorm::driver_manager::U;
use uorm::executor::mapper::Mapper;
use uorm::udbc::connection::Connection;
use uorm::udbc::sqlite::pool::SqliteDriver;

#[derive(Debug, Clone, PartialEq, Param)]
struct User {
    id: Option<i64>,
    name: Option<String>,
    age: Option<i32>,
    status: Option<String>,
    create_time: Option<String>,
}

#[derive(Param)]
struct IdArg {
    id: i64,
}

#[derive(Param)]
struct NameAgeArg {
    name: String,
    age: i32,
}

#[derive(Param)]
struct UserDetailsArg {
    id: i64,
    full: bool,
}

#[derive(Param)]
struct SearchUsersArg {
    name: String,
    min_age: i32,
}

#[derive(Param)]
struct ActiveAdultsArg {
    active: bool,
    age: i32,
}

#[derive(Param)]
struct IdsArg {
    ids: Vec<i64>,
}

#[derive(Param)]
struct UpdateAgeArg {
    id: i64,
    age: i32,
}

#[derive(Param)]
struct UpdateSelectiveArg {
    id: i64,
    name: Option<String>,
    age: Option<i32>,
}

#[derive(Param)]
struct MaxAgeArg {
    max_age: i32,
}

#[derive(Param)]
struct UserArg {
    user: User,
}

#[derive(Param)]
struct UsersArg {
    users: Vec<User>,
}

static INIT: Once = Once::new();

fn init_logger() {
    INIT.call_once(|| {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
        U.assets("tests/resources/mapper/user.xml").unwrap();
    });
}

async fn setup_mapper(base_name: &str) -> (Mapper, Box<dyn Connection>) {
    init_logger();

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let db_name = format!("{}_{}", base_name, timestamp);

    let url = format!("sqlite:file:{}?mode=memory&cache=shared", db_name);
    let driver = SqliteDriver::new(url).name(&db_name).build().unwrap();

    // 将驱动注册到 U
    U.register(driver).unwrap();

    let mapper = U.mapper_by_name(&db_name).unwrap();

    // 使用 mapper 连接池的临时连接创建表
    let mut conn = mapper.pool.acquire().await.unwrap();
    conn.execute(
        "CREATE TABLE users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT,
        age INTEGER,
        status TEXT DEFAULT 'active',
        create_time DATETIME DEFAULT CURRENT_TIMESTAMP
    )",
        &[],
    )
    .await
    .unwrap();

    (mapper, conn)
}

#[tokio::test]
async fn test_simple_select() {
    let (mapper, _conn) = setup_mapper("simple_select").await;

    // 先插入一些数据
    mapper
        .execute::<i64, _>(
            "user.insert",
            &NameAgeArg {
                name: "Alice".to_string(),
                age: 20,
            },
        )
        .await
        .unwrap();

    // 测试 get_by_id
    let users: Vec<User> = mapper
        .execute("user.get_by_id", &IdArg { id: 1 })
        .await
        .unwrap();
    println!("Get by ID result: {:?}", users);
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name.as_deref(), Some("Alice"));

    // 测试 list_all
    let users: Vec<User> = mapper.execute("user.list_all", &()).await.unwrap();
    println!("List all result: {:?}", users);
    assert_eq!(users.len(), 1);
}

#[tokio::test]
async fn test_insert_return_key() {
    let (mapper, _conn) = setup_mapper("insert_return_key").await;

    // 测试 insert（returnKey=true）
    let id: i64 = mapper
        .execute(
            "user.insert_return_key",
            &NameAgeArg {
                name: "Eve".to_string(),
                age: 28,
            },
        )
        .await
        .unwrap();

    println!("Inserted ID: {}", id);
    assert!(id > 0);

    // 校验查询结果
    let users: Vec<User> = mapper
        .execute("user.get_by_id", &IdArg { id })
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name.as_deref(), Some("Eve"));
}

#[tokio::test]
async fn test_insert_return_key_in_transaction() {
    let (mapper, _conn) = setup_mapper("insert_return_key_tx").await;

    let session = uorm::driver_manager::U
        .session_by_name(mapper.pool.name())
        .unwrap();
    let id: i64 = uorm::executor::session::with_tx_context(|| async {
        session.begin().await.unwrap();

        let id: i64 = mapper
            .execute(
                "user.insert_return_key",
                &NameAgeArg {
                    name: "Frank".to_string(),
                    age: 40,
                },
            )
            .await
            .unwrap();

        println!("Inserted ID inside TX: {}", id);
        assert!(id > 0);

        session.commit().await.unwrap();
        id
    })
    .await;

    // 校验查询结果
    let users: Vec<User> = mapper
        .execute("user.get_by_id", &IdArg { id })
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name.as_deref(), Some("Frank"));
}

#[tokio::test]
async fn test_insert() {
    let (mapper, _conn) = setup_mapper("insert").await;

    // 测试 insert_user（对象属性访问）
    let new_user = User {
        id: Some(10),
        name: Some("Bob".to_string()),
        age: Some(25),
        status: None,
        create_time: None,
    };
    let affected: i64 = mapper
        .execute("user.insert_user", &UserArg { user: new_user })
        .await
        .unwrap();
    println!("Insert user affected: {}", affected);

    let user: Vec<User> = mapper
        .execute("user.get_by_id", &IdArg { id: 10 })
        .await
        .unwrap();
    println!("Get inserted user: {:?}", user);
    assert_eq!(user.len(), 1);
    assert_eq!(user[0].name.as_deref(), Some("Bob"));

    // 测试 batch_insert（foreach）
    let users_to_insert = vec![
        User {
            id: None,
            name: Some("Charlie".to_string()),
            age: Some(30),
            status: None,
            create_time: None,
        },
        User {
            id: None,
            name: Some("David".to_string()),
            age: Some(35),
            status: None,
            create_time: None,
        },
    ];
    let batch_affected: i64 = mapper
        .execute(
            "user.batch_insert",
            &UsersArg {
                users: users_to_insert,
            },
        )
        .await
        .unwrap();
    println!("Batch insert affected: {}", batch_affected);

    let all: Vec<User> = mapper.execute("user.list_all", &()).await.unwrap();
    println!("All users after batch insert: {:?}", all);
    assert_eq!(all.len(), 3); // Bob、Charlie、David
}

#[tokio::test]
async fn test_conditional_select() {
    let (mapper, _conn) = setup_mapper("conditional_select").await;

    // 插入测试数据
    mapper
        .execute::<i64, _>(
            "user.insert",
            &NameAgeArg {
                name: "Alice".to_string(),
                age: 20,
            },
        )
        .await
        .unwrap();
    mapper
        .execute::<i64, _>(
            "user.insert",
            &NameAgeArg {
                name: "Bob".to_string(),
                age: 25,
            },
        )
        .await
        .unwrap();

    // 测试 get_user_details（full = true）
    let details: Vec<User> = mapper
        .execute(
            "user.get_user_details",
            &UserDetailsArg { id: 1, full: true },
        )
        .await
        .unwrap();
    println!("User details (full=true): {:?}", details);
    assert!(details[0].status.is_some());

    // 测试 get_user_details（full = false）
    let details_simple: Vec<User> = mapper
        .execute(
            "user.get_user_details",
            &UserDetailsArg { id: 1, full: false },
        )
        .await
        .unwrap();
    println!("User details (full=false): {:?}", details_simple);
    assert!(details_simple[0].status.is_none());

    // 测试 search_users（多个可选过滤条件）
    let searched: Vec<User> = mapper
        .execute(
            "user.search_users",
            &SearchUsersArg {
                name: "%o%".to_string(),
                min_age: 20,
            },
        )
        .await
        .unwrap();
    println!("Search users result: {:?}", searched);
    assert_eq!(searched.len(), 1);
    assert_eq!(searched[0].name.as_deref(), Some("Bob"));

    // 测试 list_active_adults（逻辑运算）
    let active_adults: Vec<User> = mapper
        .execute(
            "user.list_active_adults",
            &ActiveAdultsArg {
                active: true,
                age: 18,
            },
        )
        .await
        .unwrap();
    println!("Active adults result: {:?}", active_adults);
    assert_eq!(active_adults.len(), 2);
}

#[tokio::test]
async fn test_foreach() {
    let (mapper, _conn) = setup_mapper("foreach").await;

    mapper
        .execute::<i64, _>(
            "user.insert",
            &NameAgeArg {
                name: "Alice".to_string(),
                age: 20,
            },
        )
        .await
        .unwrap();
    mapper
        .execute::<i64, _>(
            "user.insert",
            &NameAgeArg {
                name: "Bob".to_string(),
                age: 25,
            },
        )
        .await
        .unwrap();
    mapper
        .execute::<i64, _>(
            "user.insert",
            &NameAgeArg {
                name: "Charlie".to_string(),
                age: 30,
            },
        )
        .await
        .unwrap();

    // 测试 list_by_ids
    let ids_users: Vec<User> = mapper
        .execute("user.list_by_ids", &IdsArg { ids: vec![1, 3] })
        .await
        .unwrap();
    println!("List by IDs result: {:?}", ids_users);
    assert_eq!(ids_users.len(), 2);
    assert_eq!(ids_users[0].name.as_deref(), Some("Alice"));
    assert_eq!(ids_users[1].name.as_deref(), Some("Charlie"));
}

#[tokio::test]
async fn test_update_delete() {
    let (mapper, _conn) = setup_mapper("update_delete").await;

    mapper
        .execute::<i64, _>(
            "user.insert",
            &NameAgeArg {
                name: "Alice".to_string(),
                age: 20,
            },
        )
        .await
        .unwrap();

    // 测试 update_age
    let update_affected: i64 = mapper
        .execute("user.update_age", &UpdateAgeArg { id: 1, age: 21 })
        .await
        .unwrap();
    println!("Update age affected: {}", update_affected);
    let user1: Vec<User> = mapper
        .execute("user.get_by_id", &IdArg { id: 1 })
        .await
        .unwrap();
    assert_eq!(user1[0].age.unwrap(), 21);

    // 测试 update_user_selective
    let update_selective_affected: i64 = mapper
        .execute(
            "user.update_user_selective",
            &UpdateSelectiveArg {
                id: 1,
                name: Some("Alicia".to_string()),
                age: None,
            },
        )
        .await
        .unwrap();
    println!("Update selective affected: {}", update_selective_affected);
    let user1_updated: Vec<User> = mapper
        .execute("user.get_by_id", &IdArg { id: 1 })
        .await
        .unwrap();
    println!("User after selective update: {:?}", user1_updated);
    assert_eq!(user1_updated[0].name.as_deref(), Some("Alicia"));
    assert_eq!(user1_updated[0].age.unwrap(), 21);

    // 测试 delete_by_id
    let delete_affected: i64 = mapper
        .execute("user.delete_by_id", &IdArg { id: 1 })
        .await
        .unwrap();
    println!("Delete by ID affected: {}", delete_affected);
    let all: Vec<User> = mapper.execute("user.list_all", &()).await.unwrap();
    assert_eq!(all.len(), 0);

    // 测试 delete_by_condition
    mapper
        .execute::<i64, _>(
            "user.insert",
            &NameAgeArg {
                name: "Bob".to_string(),
                age: 25,
            },
        )
        .await
        .unwrap();
    mapper
        .execute::<i64, _>(
            "user.insert",
            &NameAgeArg {
                name: "Charlie".to_string(),
                age: 30,
            },
        )
        .await
        .unwrap();

    let delete_cond_affected: i64 = mapper
        .execute("user.delete_by_condition", &MaxAgeArg { max_age: 30 })
        .await
        .unwrap();
    println!("Delete by condition affected: {}", delete_cond_affected);
    let all: Vec<User> = mapper.execute("user.list_all", &()).await.unwrap();
    println!("Final users: {:?}", all);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name.as_deref(), Some("Charlie"));
}
