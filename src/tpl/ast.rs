use crate::udbc::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Value),
    Var(String),
    Binary(Op, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum AstNode {
    Text(String),
    Var(String),
    Include {
        refid: String,
    },
    If {
        test: Expr,
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
