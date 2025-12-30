use uorm::{Param, param, sql, ToValue, FromValue};
use uorm::udbc::value::Value;

#[derive(Debug, Param)]
struct User {
    #[param("user_id")]
    pub id: i32,
    #[param(ignore)]
    pub ignored: String,
    pub name: String,
}

#[param(id = "user_id")]
#[sql("ns.list")]
pub fn list(id: i32) -> uorm::Result<()> {
    exec!()
}

#[param(id = "user_id", name = "user_name")]
#[sql("ns.update")]
pub fn update_user(id: i32, name: String) -> uorm::Result<()> {
    exec!()
}

#[test]
fn test_derive_param() {
    let user = User {
        id: 1,
        ignored: "ignore".to_string(),
        name: "jason".to_string(),
    };
    let val = user.to_value();
    if let Value::Map(map) = val {
        assert_eq!(map.get("user_id"), Some(&Value::I32(1)));
        assert_eq!(map.get("name"), Some(&Value::Str("jason".to_string())));
        assert!(map.get("ignored").is_none());
    } else {
        panic!("Expected Map");
    }
}

// We cannot easily test `list` function execution without full setup, 
// but we can check if it compiles and if `exec!` is generated correctly.
// For now, compilation is the first step.
