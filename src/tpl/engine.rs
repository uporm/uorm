use crate::error::DbError;
use crate::tpl::render::RenderBuffer;
use crate::tpl::render_context::Context;
use crate::tpl::{cache, render};
use crate::udbc::driver::Driver;
use crate::udbc::serializer::ValueSerializer;
use crate::udbc::value::Value;

/// Renders a SQL template by substituting parameters and returning the generated SQL 
/// along with the bound parameter values.
///
/// This function handles:
/// 1. Parsing the template (with caching for performance)
/// 2. Serializing parameters into a format compatible with the database driver
/// 3. Rendering the template into a final SQL string and a list of positional parameters
pub fn render_template<T: serde::Serialize>(
    template_name: &str,
    template_content: &str,
    param: &T,
    driver: &dyn Driver,
) -> Result<(String, Vec<(String, Value)>), DbError> {
    // Retrieve the abstract syntax tree (AST) for the template, using a cache to avoid re-parsing.
    let ast = cache::get_ast(template_name, template_content);

    // Convert the provided parameters into a generic Value type for SQL execution.
    let value = param.serialize(ValueSerializer)?;
    
    // Initialize the render buffer with estimated capacity to minimize reallocations.
    let mut buf = RenderBuffer {
        sql: String::with_capacity(template_content.len()),
        params: Vec::with_capacity(10),
        driver,
        param_count: 0,
    };

    // Set up the rendering context and execute the rendering process.
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
        async fn acquire(&self) -> Result<Box<dyn Connection>, DbError> {
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

    #[test]
    fn test_whitespace_trimming() {
        // This test simulates the issue where tags with newlines cause extra vertical whitespace.
        // We use a DELETE statement as the example, but it applies to any statement.
        
        let tpl = "DELETE FROM users\n        WHERE 1=1\n        <if test=\"active\">\n            AND status = 'active'\n        </if>";
        
        let args = IfArgs {
            active: true,
            age: 0,
            name: None,
        };
        
        let (sql, _) = render_template("test_whitespace", tpl, &args, &MockDriver).unwrap();
        
        // The parser should trim the body of the <if> tag:
        // "\n            AND status = 'active'\n        " 
        // becomes "AND status = 'active'"
        
        // So the result should be:
        // "DELETE FROM users\n        WHERE 1=1\n        AND status = 'active'"
        //
        // Without trimming, it would have been:
        // "DELETE FROM users\n        WHERE 1=1\n        \n            AND status = 'active'\n        "
        
        println!("Generated SQL: [{}]", sql);
        
        assert!(sql.contains("WHERE 1=1\n        AND status = 'active'"));
        assert!(!sql.contains("\n\n"));
    }

    #[test]
    fn test_blank_lines_between_tags() {
        // This test reproduces the issue where whitespace between tags accumulates
        // when the tags don't render anything.
        let tpl = r#"SELECT id, name, age FROM users 
        WHERE 1=1 
        <if test="name != null"> 
          AND name LIKE #{name} 
        </if> 
        <if test="min_age != null"> 
          AND age >= #{min_age} 
        </if> 
        <if test="max_age != null"> 
          AND age <= #{max_age} 
        </if> 
        ORDER BY id"#;

        // We need a helper to run this since render_template expects a Serialize type,
        // but we constructed a Value manually.
        // Actually, render_template calls serialize on the input. 
        // We can just define a struct or use a Map.
        
        #[derive(Serialize)]
        struct Args {
            name: Option<String>,
            min_age: Option<i32>,
            max_age: Option<i32>,
        }
        
        let args = Args {
            name: None,
            min_age: Some(18),
            max_age: None,
        };

        let (sql, _) = render_template("test_blank_lines", tpl, &args, &MockDriver).unwrap();
        
        println!("Generated SQL:\n[{}]", sql);
        
        // We want to ensure there are no blank lines (double newlines).
        // The expected output should look like:
        // ...
        // AND age >= ?
        // ORDER BY id
        
        // Current behavior might produce:
        // AND age >= ?
        // 
        // ORDER BY id
        
        let strict_double_newline = sql.contains("\n\n");
        
        assert!(!strict_double_newline, "Found double newline");
        // We might need a regex or smarter check for "blank line with whitespace"
        // But simply checking that we don't have excessive vertical space is the goal.
        
        // Let's normalize whitespace to check structure
        let lines: Vec<&str> = sql.lines().filter(|l| !l.trim().is_empty()).collect();
        // Should be: SELECT..., WHERE..., AND..., ORDER...
        assert_eq!(lines.len(), 4);
    }
}
