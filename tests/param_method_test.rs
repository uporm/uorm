use uorm::udbc::value::Value;
use uorm_macros::{param, sql};

struct UserDao;

impl UserDao {
    pub const NAMESPACE: &'static str = "UserDao";

    #[param(id = "user_id", name = "user_name")]
    #[sql("select_user")]
    pub async fn select_user(&self, id: i32, name: String) -> uorm::Result<Vec<Value>> {
        exec!()
    }
}

#[test]
fn test_method_compilation() {
    // This test primarily ensures that the macros expand to valid code
    // and that #[param] works in conjunction with #[sql].
    let _dao = UserDao;
}
