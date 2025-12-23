use crate::tpl::ast::{AstNode, Expr, Op};
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

fn is_truthy(v: &Value) -> bool {
    !matches!(v, Value::Null | Value::Bool(false))
}

fn resolve_val(expr: &Expr, ctx: &Context) -> Value {
    match expr {
        Expr::Literal(v) => v.clone(),
        Expr::Var(name) => ctx.lookup(name).clone(),
        Expr::Binary(..) => Value::Bool(eval_expr(expr, ctx)),
    }
}

pub fn eval_expr(expr: &Expr, ctx: &Context) -> bool {
    match expr {
        Expr::Binary(op, left, right) => {
            if *op == Op::And {
                return eval_expr(left, ctx) && eval_expr(right, ctx);
            }
            if *op == Op::Or {
                return eval_expr(left, ctx) || eval_expr(right, ctx);
            }

            let l_val = resolve_val(left, ctx);
            let r_val = resolve_val(right, ctx);
            let l_f64 = to_f64(&l_val);
            let r_f64 = to_f64(&r_val);

            match op {
                Op::Eq => {
                    if let (Some(l), Some(r)) = (l_f64, r_f64) {
                        (l - r).abs() < f64::EPSILON
                    } else {
                        l_val == r_val
                    }
                }
                Op::Ne => {
                    if let (Some(l), Some(r)) = (l_f64, r_f64) {
                        (l - r).abs() > f64::EPSILON
                    } else {
                        l_val != r_val
                    }
                }
                Op::Gt => l_f64.zip(r_f64).is_some_and(|(l, r)| l > r),
                Op::Ge => l_f64.zip(r_f64).is_some_and(|(l, r)| l >= r),
                Op::Lt => l_f64.zip(r_f64).is_some_and(|(l, r)| l < r),
                Op::Le => l_f64.zip(r_f64).is_some_and(|(l, r)| l <= r),
                _ => false,
            }
        }
        Expr::Literal(v) => is_truthy(v),
        Expr::Var(name) => is_truthy(ctx.lookup(name)),
    }
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
    fn test_eval_expr_logic() {
        let mut map = HashMap::new();
        map.insert("a".to_string(), Value::I64(10));
        map.insert("b".to_string(), Value::Bool(true));
        let root = Value::Map(map);
        let ctx = Context::new(&root);

        // a == 10
        let expr = Expr::Binary(
            Op::Eq,
            Box::new(Expr::Var("a".to_string())),
            Box::new(Expr::Literal(Value::I64(10))),
        );
        assert!(eval_expr(&expr, &ctx));

        // a > 5
        let expr = Expr::Binary(
            Op::Gt,
            Box::new(Expr::Var("a".to_string())),
            Box::new(Expr::Literal(Value::I64(5))),
        );
        assert!(eval_expr(&expr, &ctx));

        // b
        let expr = Expr::Var("b".to_string());
        assert!(eval_expr(&expr, &ctx));
    }
}
