use crate::error::DbError;
use crate::tpl::render::RenderBuffer;
use crate::tpl::render_context::Context;
use crate::tpl::{cache, render};
use crate::udbc::driver::Driver;
use crate::udbc::serializer::try_to_value;
use crate::udbc::value::Value;

/// 渲染模板，返回 SQL 和参数
pub fn render_template<T: serde::Serialize>(
    template_name: &str,
    template_content: &str,
    param: &T,
    driver: &dyn Driver,
) -> Result<(String, Vec<(String, Value)>), DbError> {
    // 获取 AST（缓存）
    let ast = cache::get_ast(template_name, template_content);

    // 序列化参数为 Value
    let value = try_to_value(param)?;

    // 创建渲染上下文
    let mut buf = RenderBuffer {
        sql: String::with_capacity(template_content.len()),
        params: Vec::with_capacity(10),
        driver,
        param_count: 0,
    };

    let mut ctx = Context::new(&value);
    render::render(&ast, &mut ctx, &mut buf);

    Ok((buf.sql, buf.params))
}

// pub fn remove_template(template_name: &str) {
//     cache::TEMPLATE_CACHE.remove(template_name);
// }

#[cfg(test)]
mod tests {
    use crate::error::DbError;
    use crate::tpl::engine::render_template;
    use crate::udbc::connection::Connection;
    use crate::udbc::driver::Driver;
    use crate::udbc::value::Value;
    use serde::Serialize;
    use std::sync::Arc;

    struct MockDriver;
    #[async_trait::async_trait]
    impl Driver for MockDriver {
        fn name(&self) -> &str {
            "mock"
        }

        fn r#type(&self) -> &str {
            todo!()
        }

        fn placeholder(&self, _seq: usize, _name: &str) -> String {
            "?".to_string()
        }
        async fn connection(&self) -> Result<Arc<dyn Connection>, DbError> {
            todo!()
        }
        async fn close(&self) -> Result<(), DbError> {
            todo!()
        }
    }

    #[derive(Serialize)]
    struct User {
        name: String,
        age: u8,
    }

    #[test]
    fn test_render_simple_sql() {
        let tpl = "select * from user where name = #{name} and age = #{age}";
        let user = User {
            name: "test".to_string(),
            age: 18,
        };
        let driver = MockDriver;

        let (sql, params) = render_template("test_simple", tpl, &user, &driver).unwrap();

        assert_eq!(sql, "select * from user where name = ? and age = ?");
        assert_eq!(params.len(), 2);

        assert_eq!(params[0].0, "name");
        match &params[0].1 {
            Value::Str(s) => assert_eq!(s, "test"),
            _ => panic!("Expected string"),
        }

        assert_eq!(params[1].0, "age");
        match &params[1].1 {
            Value::U8(v) => assert_eq!(v, &18),
            Value::I32(v) => assert_eq!(v, &18),
            Value::I64(v) => assert_eq!(v, &18),
            _ => {
                // fallback
            }
        }
    }

    #[derive(Serialize)]
    struct IfArgs {
        active: bool,
        age: i32,
        name: Option<String>,
    }

    #[test]
    fn test_if_tag() {
        let tpl = "select * from user where 1=1<if test=\"active\"> and status = 1</if><if test=\"age >= 18\"> and type = 'adult'</if><if test=\"name != null\"> and name = #{name}</if>";

        // Case 1: active=true, age=20, name="tom"
        let args = IfArgs {
            active: true,
            age: 20,
            name: Some("tom".to_string()),
        };
        let (sql, params) = render_template("test_if_1", tpl, &args, &MockDriver).unwrap();
        assert_eq!(
            sql,
            "select * from user where 1=1 and status = 1 and type = 'adult' and name = ?"
        );
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].0, "name");

        // Case 2: active=false, age=10, name=null
        let args = IfArgs {
            active: false,
            age: 10,
            name: None,
        };
        let (sql, params) = render_template("test_if_2", tpl, &args, &MockDriver).unwrap();
        assert_eq!(sql, "select * from user where 1=1");
        assert_eq!(params.len(), 0);
    }

    #[derive(Serialize)]
    struct ForeachArgs {
        ids: Vec<i32>,
    }

    #[test]
    fn test_foreach_tag() {
        let tpl = "select * from user where id in <foreach item=\"id\" collection=\"ids\" open= \"(\" separator=\",\" close=\")\">#{id}</foreach>";

        let args = ForeachArgs { ids: vec![1, 2, 3] };

        let (sql, params) = render_template("test_for", tpl, &args, &MockDriver).unwrap();
        assert_eq!(sql, "select * from user where id in (?,?,?)");
        assert_eq!(params.len(), 3);

        // Check params values
        for (i, val) in params.iter().enumerate() {
            assert_eq!(val.0, "id");
            match &val.1 {
                Value::I32(v) => assert_eq!(*v, args.ids[i]),
                Value::I64(v) => assert_eq!(*v, args.ids[i] as i64),
                _ => panic!("Expected integer"),
            }
        }

        // Empty list
        let args = ForeachArgs { ids: vec![] };
        let (sql, params) = render_template("test_for_empty", tpl, &args, &MockDriver).unwrap();
        assert_eq!(sql, "select * from user where id in "); // Note: usually empty IN clause is invalid SQL, but engine renders what's asked
        assert_eq!(params.len(), 0);
    }

    #[derive(Serialize)]
    struct NestedUser {
        name: String,
        roles: Vec<Role>,
    }

    #[derive(Serialize)]
    struct Role {
        id: i32,
        name: String,
    }

    #[test]
    fn test_nested_loop() {
        let tpl = "insert into user_roles (user, role) values <foreach item=\"r\" collection=\"roles\" separator=\",\">(#{name}, #{r.id})</foreach>";

        let user = NestedUser {
            name: "alice".to_string(),
            roles: vec![
                Role {
                    id: 1,
                    name: "admin".to_string(),
                },
                Role {
                    id: 2,
                    name: "editor".to_string(),
                },
            ],
        };

        let (sql, params) = render_template("test_nested", tpl, &user, &MockDriver).unwrap();
        // Expected: insert into user_roles (user, role) values (?, ?), (?, ?)
        assert_eq!(
            sql,
            "insert into user_roles (user, role) values (?, ?),(?, ?)"
        );
        assert_eq!(params.len(), 4);

        assert_eq!(params[0].1, Value::Str("alice".to_string()));
        match &params[1].1 {
            Value::I32(v) => assert_eq!(*v, 1),
            Value::I64(v) => assert_eq!(*v, 1),
            _ => panic!("Expected 1"),
        }
        assert_eq!(params[2].1, Value::Str("alice".to_string()));
        match &params[3].1 {
            Value::I32(v) => assert_eq!(*v, 2),
            Value::I64(v) => assert_eq!(*v, 2),
            _ => panic!("Expected 2"),
        }
    }
}
