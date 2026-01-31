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
        // 1) 尝试精确匹配（locals 或根对象的直接 key）
        if let Some(v) = self.get_from_scope(key) {
            return v;
        }

        // 2) 尝试点路径查找（如 "user.name"）
        if let Some((head, rest)) = key.split_once('.') {
            // 解析第一段
            if let Some(head_value) = self.get_from_scope(head) {
                // 再解析剩余路径
                if let Some(target) = Self::resolve_path(head_value, rest) {
                    return target;
                }
            }
        }

        &Value::Null
    }

    fn get_from_scope(&self, key: &str) -> Option<&'a Value> {
        // 1. 尝试精确匹配
        if let Some(v) = self.find_exact(key) {
            return Some(v);
        }

        // 2. 尝试将 key 从 camelCase 转为 snake_case
        if let Some(snake_key) = to_snake_case(key) {
            return self.find_exact(&snake_key);
        }

        None
    }

    /// 在 locals 或 root 中按精确 key 查找值
    fn find_exact(&self, key: &str) -> Option<&'a Value> {
        // 1. 优先本地变量（栈结构，反向搜索以支持遮蔽）
        if let Some((_, v)) = self.locals.iter().rev().find(|(k, _)| k == key) {
            return Some(v);
        }

        // 2. 搜索根对象
        if let Value::Map(m) = self.root {
            return m.get(key);
        }

        None
    }

    /// 在 `Value` 中解析点分路径（仅支持 Map）。
    fn resolve_path(mut current: &'a Value, path: &str) -> Option<&'a Value> {
        for part in path.split('.') {
            match current {
                Value::Map(m) => {
                    if let Some(v) = m.get(part) {
                        current = v;
                    } else if let Some(snake_part) = to_snake_case(part) {
                        // 尝试 snake_case 兜底
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

/// 将 camelCase 字符串转换为 snake_case。
/// 若字符串不包含大写字母（无需转换）则返回 None。
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

        // "a.b" 应该在 locals 中按精确匹配找到
        assert_eq!(ctx.lookup("a.b"), &Value::I64(3));
    }

    #[test]
    fn test_lookup_camel_to_snake() {
        let mut map = HashMap::new();
        map.insert("tenant_id".to_string(), Value::U64(123));
        let root = Value::Map(map);
        let ctx = Context::new(&root);

        // 查找 "tenantId" 时应命中 "tenant_id"
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

        // "userProfile.firstName" 解析为 "user_profile" 再到 "first_name"
        assert_eq!(
            ctx.lookup("userProfile.firstName"),
            &Value::Str("John".to_string())
        );
    }
}
