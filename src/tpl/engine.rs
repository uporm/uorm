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
    render::render(&ast, &mut ctx, &mut buf);

    Ok((buf.sql, buf.params))
}

// pub fn remove_template(template_name: &str) {
//     cache::TEMPLATE_CACHE.remove(template_name);
// }

