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
    Foreach {
        item: String,
        collection: String,
        open: String,
        separator: String,
        close: String,
        body: Vec<AstNode>,
    },
}
