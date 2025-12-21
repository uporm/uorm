#![cfg(feature = "sqlite")]

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use uorm::driver_manager::UORM;
use uorm::mapper_loader;
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
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER NOT NULL)",
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
async fn demo_mapper_load_from_file_and_list() {
    #[derive(Serialize)]
    struct NoArgs {}

    #[derive(Serialize)]
    struct NewUser<'a> {
        name: &'a str,
        age: i64,
    }

    mapper_loader::clear();

    let (db_name, db_path) = setup_sqlite("demo_mapper_load_from_file_and_list").await;
    UORM.assets("tests/resources/mapper/test.xml").unwrap();
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

    let mapper = UORM.mapper(&db_name).unwrap();
    let rows: Vec<User> = mapper.execute("test_ns.selectUser", &NoArgs {}).await.unwrap();

    let names: BTreeSet<String> = rows.into_iter().map(|u| u.name).collect();
    assert_eq!(
        names,
        BTreeSet::from(["alice".to_string(), "bob".to_string()])
    );

    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
async fn demo_mapper_crud_with_generated_keys_and_args() {
    #[derive(Serialize)]
    struct NewUser<'a> {
        id: i64,
        name: &'a str,
        age: i64,
    }

    #[derive(Serialize)]
    struct IdArg {
        id: i64,
    }

    #[derive(Serialize)]
    struct MinAgeArg {
        age: i64,
    }

    #[derive(Serialize)]
    struct UpdateAgeArg {
        id: i64,
        age: i64,
    }

    mapper_loader::clear();

    let xml = r#"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE mapper PUBLIC "-//uporm.github.io//DTD Mapper 1//EN" "https://uporm.github.io/dtd/uorm-1-mapper.dtd">
<mapper namespace="demo_user">
  <insert id="insert_user">
    INSERT INTO users(id, name, age) VALUES (#{id}, #{name}, #{age})
  </insert>
  <select id="get_by_id">
    SELECT id, name, age FROM users WHERE id = #{id}
  </select>
  <select id="list_all">
    SELECT id, name, age FROM users ORDER BY id
  </select>
  <select id="list_by_min_age">
    SELECT id, name, age FROM users WHERE age >= #{age} ORDER BY age
  </select>
  <update id="update_age">
    UPDATE users SET age = #{age} WHERE id = #{id}
  </update>
  <delete id="delete_by_id">
    DELETE FROM users WHERE id = #{id}
  </delete>
</mapper>
"#;

    mapper_loader::load_assets(vec![("inline_demo_user.xml", xml)]).unwrap();

    let (db_name, db_path) = setup_sqlite("demo_mapper_crud_with_generated_keys_and_args").await;
    create_users_table(&db_name).await;

    let mapper = UORM.mapper(&db_name).unwrap();

    let affected: i64 = mapper
        .execute(
            "demo_user.insert_user",
            &NewUser {
                id: 1,
                name: "alice",
                age: 18,
            },
        )
        .await
        .unwrap();
    assert_eq!(affected, 1);

    let affected: i64 = mapper
        .execute(
            "demo_user.insert_user",
            &NewUser {
                id: 2,
                name: "bob",
                age: 20,
            },
        )
        .await
        .unwrap();
    assert_eq!(affected, 1);

    let alice: User = mapper
        .execute("demo_user.get_by_id", &IdArg { id: 1 })
        .await
        .unwrap();
    assert_eq!(alice.name, "alice");
    assert_eq!(alice.age, 18);

    let older: Vec<User> = mapper
        .execute("demo_user.list_by_min_age", &MinAgeArg { age: 19 })
        .await
        .unwrap();
    assert_eq!(older.len(), 1);
    assert_eq!(older[0].name, "bob");

    let affected: i64 = mapper
        .execute("demo_user.update_age", &UpdateAgeArg { id: 1, age: 21 })
        .await
        .unwrap();
    assert_eq!(affected, 1);

    let alice: User = mapper
        .execute("demo_user.get_by_id", &IdArg { id: 1 })
        .await
        .unwrap();
    assert_eq!(alice.age, 21);

    let affected: i64 = mapper
        .execute("demo_user.delete_by_id", &IdArg { id: 2 })
        .await
        .unwrap();
    assert_eq!(affected, 1);

    #[derive(Serialize)]
    struct NoArgs {}

    let all: Vec<User> = mapper.execute("demo_user.list_all", &NoArgs {}).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id, 1);

    let _ = std::fs::remove_file(db_path);
}
