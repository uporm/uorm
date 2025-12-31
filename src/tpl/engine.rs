use crate::Result;
use crate::tpl::render::RenderBuffer;
use crate::tpl::render_context::Context;
use crate::tpl::{cache, render};
use crate::udbc::driver::Driver;
use crate::udbc::value::{ToValue, Value};

/// Renders a SQL template by substituting parameters and returning the generated SQL
/// along with the bound parameter values.
///
/// This function handles:
/// 1. Parsing the template (with caching for performance)
/// 2. Serializing parameters into a format compatible with the database driver
/// 3. Rendering the template into a final SQL string and a list of positional parameters
pub fn render_template<T: ToValue>(
    template_name: &str,
    template_content: &str,
    param: &T,
    driver: &dyn Driver,
) -> Result<(String, Vec<(String, Value)>)> {
    // Retrieve the abstract syntax tree (AST) for the template, using a cache to avoid re-parsing.
    let ast = cache::get_ast(template_name, template_content);

    // Convert the provided parameters into a generic Value type for SQL execution.
    let value = param.to_value();

    // Initialize the render buffer with estimated capacity to minimize reallocations.
    let mut buf = RenderBuffer {
        sql: String::with_capacity(template_content.len()),
        params: Vec::with_capacity(10),
        driver,
        param_count: 0,
    };

    // Set up the rendering context and execute the rendering process.
    let mut ctx = Context::new(&value);
    render::render(template_name, &ast, &mut ctx, &mut buf);

    Ok((buf.sql, buf.params))
}

// pub fn remove_template(template_name: &str) {
//     cache::TEMPLATE_CACHE.remove(template_name);
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tpl::cache;
    use crate::udbc::connection::Connection;
    use async_trait::async_trait;

    struct TestDriver;

    #[async_trait]
    impl Driver for TestDriver {
        fn name(&self) -> &str {
            "test"
        }

        fn r#type(&self) -> &str {
            "test"
        }

        fn placeholder(&self, _param_seq: usize, _param_name: &str) -> String {
            "?".to_string()
        }

        async fn acquire(&self) -> Result<Box<dyn Connection>> {
            Err(crate::error::DbError::DbError("not supported".to_string()))
        }

        async fn close(&self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn include_is_resolved_by_current_namespace_first() {
        cache::TEMPLATE_CACHE.clear();

        cache::get_ast("a.cols", "id, name");
        cache::get_ast("b.cols", "id, email");
        cache::get_ast("cols", "WRONG");

        let driver = TestDriver;
        let (sql, _params) = render_template(
            "a.main",
            "select <include refid=\"cols\"/> from t",
            &(),
            &driver,
        )
        .unwrap();
        assert!(sql.contains("id, name"));
        assert!(!sql.contains("WRONG"));

        let (sql, _params) = render_template(
            "b.main",
            "select <include refid=\"cols\"/> from t",
            &(),
            &driver,
        )
        .unwrap();
        assert!(sql.contains("id, email"));
        assert!(!sql.contains("WRONG"));

        let (sql, _params) = render_template(
            "a.main2",
            "select <include refid=\"b.cols\"/> from t",
            &(),
            &driver,
        )
        .unwrap();
        assert!(sql.contains("id, email"));
    }
}
