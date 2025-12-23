use crate::udbc::value::Value;

pub struct Context<'a> {
    root: &'a Value,
    locals: Vec<(String, &'a Value)>,
}

impl<'a> Context<'a> {
    pub fn new(root: &'a Value) -> Self {
        Self {
            root,
            locals: Vec::new(),
        }
    }

    pub fn push(&mut self, key: &str, value: &'a Value) {
        self.locals.push((key.to_string(), value));
    }

    pub fn pop(&mut self) {
        self.locals.pop();
    }

    pub fn lookup(&self, key: &str) -> &'a Value {
        // 1) Try an exact match (locals or a direct key on the root object).
        if let Some(v) = self.get_from_scope(key) {
            return v;
        }

        // 2) Try dotted-path lookup (e.g. "user.name").
        if let Some((head, rest)) = key.split_once('.') {
            // Resolve the first segment.
            if let Some(head_value) = self.get_from_scope(head) {
                // Then resolve the remaining path.
                if let Some(target) = Self::resolve_path(head_value, rest) {
                    return target;
                }
            }
        }

        &Value::Null
    }

    fn get_from_scope(&self, key: &str) -> Option<&'a Value> {
        // 1. Prioritize local variables (Stack structure, search backwards to support shadowing)
        // Complexity: O(L) where L is the depth of nested loops/scopes.
        // Since L is typically small (< 10), linear scan is faster than HashMap overhead.
        if let Some((_, v)) = self.locals.iter().rev().find(|(k, _)| k == key) {
            return Some(v);
        }

        // 2. Search root object
        if let Value::Map(m) = self.root {
            return m.get(key);
        }

        None
    }

    /// Resolve a dot-separated path within a `Value` (maps only).
    fn resolve_path(mut current: &'a Value, path: &str) -> Option<&'a Value> {
        for part in path.split('.') {
            match current {
                Value::Map(m) => {
                    current = m.get(part)?;
                }
                _ => return None,
            }
        }
        Some(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_lookup_simple() {
        let mut map = HashMap::new();
        map.insert("a".to_string(), Value::I64(1));
        let root = Value::Map(map);
        let ctx = Context::new(&root);

        assert_eq!(ctx.lookup("a"), &Value::I64(1));
        assert_eq!(ctx.lookup("b"), &Value::Null);
    }

    #[test]
    fn test_lookup_nested() {
        let mut sub = HashMap::new();
        sub.insert("b".to_string(), Value::I64(2));

        let mut map = HashMap::new();
        map.insert("a".to_string(), Value::Map(sub));
        let root = Value::Map(map);
        let ctx = Context::new(&root);

        assert_eq!(ctx.lookup("a.b"), &Value::I64(2));
        assert_eq!(ctx.lookup("a.c"), &Value::Null);
        assert_eq!(ctx.lookup("x.y"), &Value::Null);
    }

    #[test]
    fn test_lookup_locals_shadowing() {
        let mut map = HashMap::new();
        map.insert("a".to_string(), Value::I64(1));
        let root = Value::Map(map);
        let mut ctx = Context::new(&root);

        ctx.push("a", &Value::I64(2));
        assert_eq!(ctx.lookup("a"), &Value::I64(2));

        ctx.pop();
        assert_eq!(ctx.lookup("a"), &Value::I64(1));
    }

    #[test]
    fn test_lookup_exact_match_with_dot() {
        let mut map = HashMap::new();
        map.insert("a".to_string(), Value::I64(1));
        let root = Value::Map(map);
        let mut ctx = Context::new(&root);

        ctx.push("a.b", &Value::I64(3));

        // "a.b" should be found in locals as exact match
        assert_eq!(ctx.lookup("a.b"), &Value::I64(3));
    }
}
