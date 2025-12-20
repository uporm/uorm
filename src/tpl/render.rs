use crate::tpl::AstNode;
use crate::tpl::cache::TEMPLATE_CACHE;
use crate::tpl::render_context::Context;
use crate::udbc::driver::Driver;
use crate::udbc::value::Value;

pub struct RenderBuffer<'a> {
    pub sql: String,
    pub params: Vec<(String, Value)>,
    pub driver: &'a dyn Driver,
    pub param_count: usize,
}

fn to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::I16(n) => Some(*n as f64),
        Value::I32(n) => Some(*n as f64),
        Value::I64(n) => Some(*n as f64),
        Value::U8(n) => Some(*n as f64),
        Value::F64(n) => Some(*n),
        _ => None,
    }
}

fn eval_atom(expr: &str, ctx: &Context) -> bool {
    let expr = expr.trim();
    if expr.is_empty() {
        return false;
    }

    // Split by operator (check longest operators first)
    let (key, op, val_str) = if let Some((k, v)) = expr.split_once("!=") {
        (k.trim(), "!=", v.trim())
    } else if let Some((k, v)) = expr.split_once("==") {
        (k.trim(), "==", v.trim())
    } else if let Some((k, v)) = expr.split_once(">=") {
        (k.trim(), ">=", v.trim())
    } else if let Some((k, v)) = expr.split_once("<=") {
        (k.trim(), "<=", v.trim())
    } else if let Some((k, v)) = expr.split_once(">") {
        (k.trim(), ">", v.trim())
    } else if let Some((k, v)) = expr.split_once("<") {
        (k.trim(), "<", v.trim())
    } else {
        let val = ctx.lookup(expr);
        return !matches!(val, Value::Null | Value::Bool(false));
    };

    let left = ctx.lookup(key);

    let right_owned;
    let right = if val_str == "null" {
        &Value::Null
    } else if val_str == "true" {
        &Value::Bool(true)
    } else if val_str == "false" {
        &Value::Bool(false)
    } else if (val_str.starts_with('\'') && val_str.ends_with('\''))
        || (val_str.starts_with('"') && val_str.ends_with('"'))
    {
        right_owned = Value::Str(val_str[1..val_str.len() - 1].to_string());
        &right_owned
    } else if let Ok(n) = val_str.parse::<i64>() {
        right_owned = Value::I64(n);
        &right_owned
    } else if let Ok(n) = val_str.parse::<f64>() {
        right_owned = Value::F64(n);
        &right_owned
    } else {
        ctx.lookup(val_str)
    };

    match op {
        "==" => {
            if let (Some(l), Some(r)) = (to_f64(left), to_f64(right)) {
                (l - r).abs() < f64::EPSILON
            } else {
                left == right
            }
        }
        "!=" => {
            if let (Some(l), Some(r)) = (to_f64(left), to_f64(right)) {
                (l - r).abs() > f64::EPSILON
            } else {
                left != right
            }
        }
        ">" => to_f64(left).zip(to_f64(right)).is_some_and(|(l, r)| l > r),
        ">=" => to_f64(left).zip(to_f64(right)).is_some_and(|(l, r)| l >= r),
        "<" => to_f64(left).zip(to_f64(right)).is_some_and(|(l, r)| l < r),
        "<=" => to_f64(left).zip(to_f64(right)).is_some_and(|(l, r)| l <= r),
        _ => false,
    }
}

pub fn eval_expr(expr: &str, ctx: &Context) -> bool {
    for or_part in expr.split(" or ") {
        let mut and_satisfied = true;
        for atom in or_part.split(" and ") {
            if !eval_atom(atom, ctx) {
                and_satisfied = false;
                break;
            }
        }
        if and_satisfied {
            return true;
        }
    }
    false
}

pub(crate) fn render(nodes: &[AstNode], ctx: &mut Context, buf: &mut RenderBuffer) {
    for node in nodes {
        match node {
            AstNode::Text(t) => buf.sql.push_str(t),
            AstNode::Var(name) => {
                let v = ctx.lookup(name);
                buf.params.push((name.clone(), v.clone()));
                buf.param_count += 1;
                buf.sql
                    .push_str(&buf.driver.placeholder(buf.param_count, name));
            }
            AstNode::Include { refid } => {
                if let Some(cached) = TEMPLATE_CACHE.get(refid) {
                    render(&cached.ast, ctx, buf);
                }
            }
            AstNode::If { test, body } => {
                if eval_expr(test, ctx) {
                    render(body, ctx, buf);
                }
            }
            AstNode::Foreach {
                item,
                collection,
                open,
                separator,
                close,
                body,
            } => {
                let arr = match ctx.lookup(collection) {
                    Value::List(v) => v,
                    _ => continue,
                };
                if arr.is_empty() {
                    continue;
                }

                buf.sql.push_str(open);
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        buf.sql.push_str(separator);
                    }

                    ctx.push(item, v);
                    render(body, ctx, buf);
                    ctx.pop();
                }
                buf.sql.push_str(close);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_eval_atom_literals() {
        let root = Value::Map(HashMap::new());
        let ctx = Context::new(&root);

        assert!(!eval_atom("var", &ctx));

        let mut map = HashMap::new();
        map.insert("a".to_string(), Value::I64(10));
        map.insert("b".to_string(), Value::Str("hello".to_string()));
        map.insert("c".to_string(), Value::Bool(true));
        let root = Value::Map(map);
        let ctx = Context::new(&root);

        assert!(eval_atom("a == 10", &ctx));
        assert!(eval_atom("a != 5", &ctx));
        assert!(eval_atom("b == 'hello'", &ctx));
        assert!(eval_atom("b != 'world'", &ctx));
        assert!(eval_atom("c", &ctx));
        assert!(eval_atom("c == true", &ctx));

        // New comparisons
        assert!(eval_atom("a > 5", &ctx));
        assert!(eval_atom("a >= 10", &ctx));
        assert!(eval_atom("a < 20", &ctx));
        assert!(eval_atom("a <= 10", &ctx));
    }

    #[test]
    fn test_eval_expr() {
        let mut map = HashMap::new();
        map.insert("x".to_string(), Value::I64(1));
        map.insert("y".to_string(), Value::I64(2));
        let root = Value::Map(map);
        let ctx = Context::new(&root);

        assert!(eval_expr("x == 1 and y == 2", &ctx));
        assert!(eval_expr("x == 1 or y == 3", &ctx));
        assert!(!eval_expr("x == 2 or y == 3", &ctx));
    }
}
