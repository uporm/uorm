use serde::{Deserialize, Serialize};
use uorm::sql;

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    id: i64,
    name: String,
}

#[sql("user")]
struct UserDao;

impl UserDao {
    #[sql("list")]
    pub async fn list(name: String) -> Result<Vec<User>, uorm::error::DbError> {
        // Custom logic before exec
        let _ = name.len();
        exec!()
    }

    // Test with named args in macro
    #[sql(id = "get_by_id")]
    pub async fn get(id: i64) -> Result<User, uorm::error::DbError> {
        println!("Getting user with id: {}", id);
        let res = exec!();
        // Custom logic after exec
        println!("Got user");
        res
    }

    #[sql("insert")]
    pub async fn insert(user: User) -> Result<u64, uorm::error::DbError> {
        exec!()
    }

    #[sql(id = "update", database = "default")]
    pub async fn update(id: i64, name: String) -> Result<u64, uorm::error::DbError> {
        exec!()
    }

    #[sql("delete")]
    pub async fn delete(id: i64) -> Result<u64, uorm::error::DbError> {
        exec!()
    }
}

#[tokio::test]
async fn test_macros_expansion() {
    // This test primarily checks if the code compiles and expands correctly.
    // We access the namespace constant to ensure sql_namespace worked.
    assert_eq!(UserDao::NAMESPACE, "user");

    // We cannot easily call the methods because they will panic at runtime
    // due to missing database driver registration (uorm::driver_manager::UORM).
    // But compilation success is the main verification here.
}
