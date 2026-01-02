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
        // 1. Try exact match
        if let Some(v) = self.find_exact(key) {
            return Some(v);
        }

        // 2. Try converting key from camelCase to snake_case
        if let Some(snake_key) = to_snake_case(key) {
            return self.find_exact(&snake_key);
        }

        None
    }

    /// Helper to find a value by exact key match in locals or root
    fn find_exact(&self, key: &str) -> Option<&'a Value> {
        // 1. Prioritize local variables (Stack structure, search backwards to support shadowing)
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
                    if let Some(v) = m.get(part) {
                        current = v;
                    } else if let Some(snake_part) = to_snake_case(part) {
                        // Try snake_case fallback
                        if let Some(v) = m.get(&snake_part) {
                            current = v;
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        Some(current)
    }
}

/// Converts a camelCase string to snake_case.
/// Returns None if the string does not contain uppercase letters (no conversion needed).
fn to_snake_case(s: &str) -> Option<String> {
    if !s.chars().any(|c| c.is_uppercase()) {
        return None;
    }

    let mut snake = String::with_capacity(s.len() + 2);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                snake.push('_');
            }
            snake.push(c.to_ascii_lowercase());
        } else {
            snake.push(c);
        }
    }
    Some(snake)
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

    #[test]
    fn test_lookup_camel_to_snake() {
        let mut map = HashMap::new();
        map.insert("tenant_id".to_string(), Value::U64(123));
        let root = Value::Map(map);
        let ctx = Context::new(&root);

        // Should find "tenant_id" when looking up "tenantId"
        assert_eq!(ctx.lookup("tenantId"), &Value::U64(123));
    }

    #[test]
    fn test_lookup_nested_camel_to_snake() {
        let mut sub = HashMap::new();
        sub.insert("first_name".to_string(), Value::Str("John".to_string()));
        
        let mut map = HashMap::new();
        map.insert("user_profile".to_string(), Value::Map(sub));
        
        let root = Value::Map(map);
        let ctx = Context::new(&root);

        // "userProfile.firstName" -> "user_profile" -> "first_name"
        assert_eq!(ctx.lookup("userProfile.firstName"), &Value::Str("John".to_string()));
    }
}
