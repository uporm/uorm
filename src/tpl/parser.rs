use crate::tpl::ast::{AstNode, Expr, Op};
use crate::udbc::value::Value;
use std::collections::HashMap;

/// 模板解析时用于处理嵌套标签的栈帧表示。
///
/// 遇到起始标签（如 `<if>`）时，会将新的 `TagFrame` 入栈，
/// 以便解析器跟踪当前标签的属性与嵌套层级。
enum TagFrame {
    /// `<if>` 标签栈帧，保存测试表达式。
    If { test: Expr },
    /// `<foreach>` 标签栈帧，保存迭代细节。
    Foreach {
        item: String,
        collection: String,
        open: String,
        separator: String,
        close: String,
    },
}

/// SQL 模板语言的手写递归下降解析器。
///
/// 支持：
/// - 纯文本（SQL）
/// - 变量插值：`#{var}`
/// - 条件逻辑：`<if test="...">...</if>`
/// - 迭代：`<foreach item="..." collection="..." ...>...</foreach>`
/// - 模板包含：`<include refid="..." />`
///
/// 解析器使用基于栈的方法正确处理嵌套标签。
struct Parser<'a> {
    /// 正在解析的原始模板字符串。
    template: &'a str,
    /// 当前在模板中的字符位置。
    pos: usize,
    /// 节点集合栈。每一层对应一个嵌套标签的子节点集合。
    /// 第一层始终是根节点集合。
    nodes_stack: Vec<Vec<AstNode>>,
    /// 正在解析的活动标签栈。
    tag_stack: Vec<TagFrame>,
}

impl<'a> Parser<'a> {
    /// 基于给定模板字符串创建解析器实例。
    fn new(template: &'a str) -> Self {
        Self {
            template,
            pos: 0,
            nodes_stack: vec![Vec::new()], // 使用根层初始化
            tag_stack: Vec::new(),
        }
    }

    /// 解析完整模板并返回根级 `AstNode` 列表。
    fn parse(mut self) -> Vec<AstNode> {
        while self.pos < self.template.len() {
            // 先尝试解析结构化元素（标签或变量）
            if self.try_parse_tag() || self.try_parse_var() {
                continue;
            }

            // 回退：若当前位置没有结构化元素则按纯文本解析
            self.parse_text();
        }

        // 关闭所有未闭合标签（如缺少 </if>）
        self.close_remaining_tags();

        // 返回根级节点
        self.nodes_stack.pop().unwrap_or_default()
    }

    /// 尝试解析标签：`<if>`、`</if>`、`<foreach>`、`</foreach>`、`<include>`。
    /// 若标签成功解析并消费则返回 true。
    fn try_parse_tag(&mut self) -> bool {
        let remaining = &self.template[self.pos..];

        if remaining.starts_with("</") {
            return self.handle_close_tag(remaining);
        }
        if remaining.starts_with("<if ") {
            return self.handle_if_tag(remaining);
        }
        if remaining.starts_with("<foreach ") {
            return self.handle_foreach_tag(remaining);
        }
        if remaining.starts_with("<include") {
            return self.handle_include_tag(remaining);
        }

        false
    }

    /// 处理 <if test="...">
    fn handle_if_tag(&mut self, remaining: &str) -> bool {
        if let Some(end_idx) = find_tag_end(remaining) {
            let tag_content = &remaining[4..end_idx]; // 跳过 "<if "
            let attrs = parse_attributes(tag_content);
            if let Some(test_str) = attrs.get("test") {
                let test = parse_expr(test_str);
                self.nodes_stack.push(Vec::new());
                self.tag_stack.push(TagFrame::If { test });
                self.pos += end_idx + 1;
                return true;
            }
        }
        false
    }

    /// 处理 <foreach item="..." collection="...">
    fn handle_foreach_tag(&mut self, remaining: &str) -> bool {
        if let Some(end_idx) = find_tag_end(remaining) {
            let tag_content = &remaining[9..end_idx]; // 跳过 "<foreach "
            let attrs = parse_attributes(tag_content);
            if let (Some(item), Some(collection)) = (attrs.get("item"), attrs.get("collection")) {
                let open = attrs.get("open").map(|s| s.as_str()).unwrap_or("");
                let separator = attrs.get("separator").map(|s| s.as_str()).unwrap_or(",");
                let close = attrs.get("close").map(|s| s.as_str()).unwrap_or("");

                self.nodes_stack.push(Vec::new());
                self.tag_stack.push(TagFrame::Foreach {
                    item: item.to_string(),
                    collection: collection.to_string(),
                    open: open.to_string(),
                    separator: separator.to_string(),
                    close: close.to_string(),
                });
                self.pos += end_idx + 1;
                return true;
            }
        }
        false
    }

    /// 处理 <include refid="..." />
    fn handle_include_tag(&mut self, remaining: &str) -> bool {
        if let Some(end_idx) = find_tag_end(remaining) {
            let tag_content = &remaining[8..end_idx]; // 跳过 "<include"
            let attrs = parse_attributes(tag_content);
            if let Some(refid) = attrs.get("refid") {
                self.append_node(AstNode::Include {
                    refid: refid.to_string(),
                });
                self.pos += end_idx + 1;
                return true;
            }
        }
        false
    }

    /// 处理结束标签 `</if>` 与 `</foreach>`。
    fn handle_close_tag(&mut self, remaining: &str) -> bool {
        if remaining.starts_with("</if>")
            && let Some(TagFrame::If { .. }) = self.tag_stack.last()
            && let Some(TagFrame::If { test }) = self.tag_stack.pop()
        {
            let mut body = self.nodes_stack.pop().unwrap_or_default();
            self.trim_text_nodes(&mut body);

            self.append_node(AstNode::If { test, body });
            self.pos += 5;
            return true;
        } else if remaining.starts_with("</foreach>")
            && let Some(TagFrame::Foreach { .. }) = self.tag_stack.last()
            && let Some(TagFrame::Foreach {
                item,
                collection,
                open,
                separator,
                close,
            }) = self.tag_stack.pop()
        {
            let mut body = self.nodes_stack.pop().unwrap_or_default();
            self.trim_text_nodes(&mut body);

            self.append_node(AstNode::Foreach {
                item,
                collection,
                open,
                separator,
                close,
                body,
            });
            self.pos += 10;
            return true;
        }
        false
    }

    fn trim_text_nodes(&self, nodes: &mut Vec<AstNode>) {
        if let Some(AstNode::Text(text)) = nodes.first_mut() {
            let trimmed = text.trim_start();
            let whitespace_len = text.len() - trimmed.len();
            let whitespace = &text[..whitespace_len];

            // 仅在空白包含换行时裁剪（块级格式）
            // 若仅为空格（行内格式）则保留
            if whitespace.contains('\n') {
                if trimmed.is_empty() {
                    nodes.remove(0);
                } else {
                    *text = trimmed.to_string();
                }
            }
        }

        // 可能删除后再检查末尾（len=1 时可能是同一节点）
        if let Some(AstNode::Text(text)) = nodes.last_mut() {
            let trimmed = text.trim_end();
            let whitespace = &text[trimmed.len()..];

            if whitespace.contains('\n') {
                if trimmed.is_empty() {
                    nodes.pop();
                } else {
                    *text = trimmed.to_string();
                }
            }
        }
    }

    /// 尝试解析变量表达式：`#{var}`。
    fn try_parse_var(&mut self) -> bool {
        let remaining = &self.template[self.pos..];
        if remaining.starts_with("#{")
            && let Some(end) = remaining.find('}')
        {
            let var_name = remaining[2..end].trim();
            if !var_name.is_empty() {
                self.append_node(AstNode::Var(var_name.to_string()));
                self.pos += end + 1;
                return true;
            }
        }
        false
    }

    /// 消费文本直到下一个特殊序列（`'<'` 或 `"#{"`）。
    fn parse_text(&mut self) {
        let remaining = &self.template[self.pos..];
        let next_tag = remaining.find('<').unwrap_or(remaining.len());
        let next_var = remaining.find("#{").unwrap_or(remaining.len());
        let next_stop = std::cmp::min(next_tag, next_var);

        if next_stop > 0 {
            self.append_text(&remaining[..next_stop]);
            self.pos += next_stop;
        } else {
            // 要么未找到标签，要么处于未解析成功的标签/变量边界
            // 消费一个字符以推进并避免死循环
            self.append_text(&remaining[0..1]);
            self.pos += 1;
        }
    }

    /// 向当前活动作用域追加节点。
    fn append_node(&mut self, node: AstNode) {
        if let Some(nodes) = self.nodes_stack.last_mut() {
            nodes.push(node);
        }
    }

    /// 追加文本，尽可能与上一个文本节点合并。
    fn append_text(&mut self, text: &str) {
        if let Some(nodes) = self.nodes_stack.last_mut() {
            if let Some(AstNode::Text(last_text)) = nodes.last_mut() {
                last_text.push_str(text);
            } else {
                nodes.push(AstNode::Text(text.to_string()));
            }
        }
    }

    /// 在模板末尾自动闭合所有未闭合标签。
    fn close_remaining_tags(&mut self) {
        while let Some(tag) = self.tag_stack.pop() {
            let mut body = self.nodes_stack.pop().unwrap_or_default();
            self.trim_text_nodes(&mut body);

            let node = match tag {
                TagFrame::If { test } => AstNode::If { test, body },
                TagFrame::Foreach {
                    item,
                    collection,
                    open,
                    separator,
                    close,
                } => AstNode::Foreach {
                    item,
                    collection,
                    open,
                    separator,
                    close,
                    body,
                },
            };
            self.append_node(node);
        }
    }
}

/// 主入口：将模板字符串解析为 AST。
pub fn parse_template(template: &str) -> Vec<AstNode> {
    Parser::new(template).parse()
}

/// 查找标签闭合 `>` 的索引，忽略引号内内容。
fn find_tag_end(s: &str) -> Option<usize> {
    let mut in_quote = false;
    for (i, c) in s.char_indices() {
        if c == '"' {
            in_quote = !in_quote;
        } else if c == '>' && !in_quote {
            return Some(i);
        }
    }
    None
}

/// 从标签内容解析属性为 HashMap
fn parse_attributes(content: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();

    let mut rest = content;
    while !rest.is_empty() {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }

        // 查找 key 结束位置
        let key_end = rest
            .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .unwrap_or(rest.len());
        if key_end == 0 {
            // 若 trim_start 正常且字符有效，理论上不会发生
            // 若存在脏数据，消费一个字符以避免死循环
            rest = &rest[1..];
            continue;
        }
        let key = &rest[..key_end];
        rest = rest[key_end..].trim_start();

        // 期待 '='
        if !rest.starts_with('=') {
            continue;
        }
        rest = rest[1..].trim_start();

        // 期待引号
        if rest.is_empty() {
            break;
        }
        let quote = rest.chars().next().unwrap();
        if quote != '"' && quote != '\'' {
            continue;
        }
        rest = &rest[1..];

        // 查找匹配的引号
        if let Some(val_end) = rest.find(quote) {
            let val = &rest[..val_end];
            attrs.insert(key.to_string(), val.to_string());
            rest = &rest[val_end + 1..];
        } else {
            break; // 引号未闭合
        }
    }
    attrs
}

fn parse_expr(input: &str) -> Expr {
    // 1. 按 OR 拆分
    let parts: Vec<&str> = input.split(" or ").collect();
    if parts.len() > 1 {
        let mut expr = parse_and_expr(parts[0]);
        for part in &parts[1..] {
            expr = Expr::Binary(Op::Or, Box::new(expr), Box::new(parse_and_expr(part)));
        }
        return expr;
    }
    parse_and_expr(input)
}

fn parse_and_expr(input: &str) -> Expr {
    let parts: Vec<&str> = input.split(" and ").collect();
    if parts.len() > 1 {
        let mut expr = parse_atom(parts[0]);
        for part in &parts[1..] {
            expr = Expr::Binary(Op::And, Box::new(expr), Box::new(parse_atom(part)));
        }
        return expr;
    }
    parse_atom(input)
}

fn parse_atom(input: &str) -> Expr {
    let input = input.trim();
    // 检查运算符，顺序很重要（最长优先）
    let ops = [
        ("!=", Op::Ne),
        ("==", Op::Eq),
        (">=", Op::Ge),
        ("<=", Op::Le),
        (">", Op::Gt),
        ("<", Op::Lt),
    ];

    for (sym, op) in ops {
        if let Some((left, right)) = input.split_once(sym) {
            return Expr::Binary(op, Box::new(parse_val(left)), Box::new(parse_val(right)));
        }
    }

    // 隐式布尔判断
    parse_val(input)
}

fn parse_val(input: &str) -> Expr {
    let s = input.trim();
    if s == "null" {
        return Expr::Literal(Value::Null);
    }
    if s == "true" {
        return Expr::Literal(Value::Bool(true));
    }
    if s == "false" {
        return Expr::Literal(Value::Bool(false));
    }
    if (s.starts_with('\'') && s.ends_with('\'')) || (s.starts_with('"') && s.ends_with('"')) {
        return Expr::Literal(Value::Str(s[1..s.len() - 1].to_string()));
    }
    if let Ok(n) = s.parse::<i64>() {
        return Expr::Literal(Value::I64(n));
    }
    if let Ok(n) = s.parse::<f64>() {
        return Expr::Literal(Value::F64(n));
    }
    // 变量
    Expr::Var(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_text() {
        let tpl = "hello world";
        let nodes = parse_template(tpl);
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            AstNode::Text(t) => assert_eq!(t, "hello world"),
            _ => panic!("Expected Text"),
        }
    }

    #[test]
    fn test_parse_merged_text() {
        let tpl = "hello < world";
        let nodes = parse_template(tpl);
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            AstNode::Text(t) => assert_eq!(t, "hello < world"),
            _ => panic!("Expected Text"),
        }
    }

    #[test]
    fn test_parse_var() {
        let tpl = "hello #{name}!";
        let nodes = parse_template(tpl);
        assert_eq!(nodes.len(), 3);
        match &nodes[0] {
            AstNode::Text(t) => assert_eq!(t, "hello "),
            _ => panic!(),
        }
        match &nodes[1] {
            AstNode::Var(v) => assert_eq!(v, "name"),
            _ => panic!(),
        }
        match &nodes[2] {
            AstNode::Text(t) => assert_eq!(t, "!"),
            _ => panic!(),
        }
    }

    #[test]
    fn test_parse_if() {
        let tpl = r#"<if test="a > 1">content</if>"#;
        let nodes = parse_template(tpl);
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            AstNode::If { test, body } => {
                match test {
                    Expr::Binary(Op::Gt, left, right) => {
                        assert_eq!(**left, Expr::Var("a".to_string()));
                        assert_eq!(**right, Expr::Literal(Value::I64(1)));
                    }
                    _ => panic!("Expected Binary expression, got {:?}", test),
                }
                assert_eq!(body.len(), 1);
                match &body[0] {
                    AstNode::Text(t) => assert_eq!(t, "content"),
                    _ => panic!(),
                }
            }
            _ => panic!("Expected If"),
        }
    }

    #[test]
    fn test_parse_nested() {
        let tpl = r#"<if test="x"><foreach item="i" collection="list">#{i}</foreach></if>"#;
        let nodes = parse_template(tpl);
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            AstNode::If { body, .. } => {
                assert_eq!(body.len(), 1);
                match &body[0] {
                    AstNode::Foreach { item, body, .. } => {
                        assert_eq!(item, "i");
                        assert_eq!(body.len(), 1);
                    }
                    _ => panic!("Expected Foreach"),
                }
            }
            _ => panic!("Expected If"),
        }
    }

    #[test]
    fn test_auto_close() {
        let tpl = r#"<if test="x">content"#;
        let nodes = parse_template(tpl);
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            AstNode::If { test, body } => {
                match test {
                    Expr::Var(v) => assert_eq!(v, "x"),
                    _ => panic!("Expected Var"),
                }
                assert_eq!(body.len(), 1);
                match &body[0] {
                    AstNode::Text(t) => assert_eq!(t, "content"),
                    _ => panic!(),
                }
            }
            _ => panic!("Expected If"),
        }
    }

    #[test]
    fn test_malformed_tags() {
        let tpl = r#"<if test="x"> <unknown> #{ unclosed"#;
        let nodes = parse_template(tpl);
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            AstNode::If { body, .. } => {
                assert_eq!(body.len(), 1);
                match &body[0] {
                    AstNode::Text(t) => assert_eq!(t, " <unknown> #{ unclosed"),
                    _ => panic!("Expected Text, got {:?}", body[0]),
                }
            }
            _ => panic!("Expected If"),
        }
    }
}
