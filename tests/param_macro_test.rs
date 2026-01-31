use uorm::Param;
use uorm::udbc::value::Value;
use uorm::udbc::value::{FromValue, ToValue};

#[derive(Debug, PartialEq, Param)]
struct User {
    pub id: i32,
    pub name: String,
    #[param(ignore)]
    pub ignored: String,
    #[param("custom_col")]
    pub custom: String,
}

#[test]
fn test_param_struct_from_value() {
    let mut row = std::collections::HashMap::new();
    row.insert("id".to_string(), Value::I32(1));
    row.insert("name".to_string(), Value::Str("Alice".to_string()));
    row.insert("customCol".to_string(), Value::Str("CustomVal".to_string()));
    row.insert(
        "ignored".to_string(),
        Value::Str("ShouldBeIgnored".to_string()),
    );

    let user =
        <User as FromValue>::from_value(Value::Map(row)).expect("Failed to convert from value");

    assert_eq!(user.id, 1);
    assert_eq!(user.name, "Alice");
    assert_eq!(user.custom, "CustomVal");
    assert_eq!(user.ignored, ""); // 默认字符串
}

#[test]
fn test_param_struct_to_value() {
    let user = User {
        id: 2,
        name: "Bob".to_string(),
        ignored: "Secret".to_string(),
        custom: "Val".to_string(),
    };

    let val = user.to_value();

    match val {
        Value::Map(map) => {
            // 注意：Value 类型必须与 ToValue 的输出完全一致
            assert_eq!(map.get("id"), Some(&Value::I32(2)));
            assert_eq!(map.get("name"), Some(&Value::Str("Bob".to_string())));
            assert_eq!(map.get("custom_col"), Some(&Value::Str("Val".to_string())));
            assert_eq!(map.get("customCol"), Some(&Value::Str("Val".to_string())));
            // ignored 字段不应出现在 map 中
            assert!(!map.contains_key("ignored"));
        }
        _ => panic!("Expected Value::Map"),
    }
}
