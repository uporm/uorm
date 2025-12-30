use uorm::{sql, exec, Param};
use uorm::error::DbError;

#[derive(Debug, PartialEq, Param)]
pub struct UserEntity {
    pub id: i64,
    pub tenant_id: i64,
    pub name: String,
    pub email: String,
    pub password: Option<String>,
    pub owner: i16,
    pub description: Option<String>,
}

#[sql("user")]
pub struct UserDao;

impl UserDao {
    #[sql("insert")]
    pub async fn insert(user: UserEntity) -> Result<i64, DbError> {
        exec!()
    }

    #[sql("list")]
    pub async fn list(tenant_id: i64) -> uorm::Result<Vec<UserEntity>> {
        exec!()
    }
}

#[tokio::test]
async fn test_param_vec_compilation() {
    // This test primarily checks if the code above compiles successfully.
    // The specific error [E0277] for Vec<UserEntity>: MapperResult would prevent compilation.
    
    // We can also mock a runtime check if we had a DB, but compilation is the main point.
    assert!(true);
}
