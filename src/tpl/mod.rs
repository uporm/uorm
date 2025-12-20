mod cache;
pub(crate) mod engine;
mod parser;
mod render;
mod render_context;

#[derive(Debug, Clone)]
pub enum AstNode {
    Text(String),
    Var(String),
    Include {
        refid: String,
    },
    If {
        test: String,
        body: Vec<AstNode>,
    },
    For {
        item: String,
        collection: String,
        open: String,
        sep: String,
        close: String,
        body: Vec<AstNode>,
    },
}
