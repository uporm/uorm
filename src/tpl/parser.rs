use crate::tpl::AstNode;

/// 用于跟踪嵌套标签（如 <if> 和 <foreach>）的栈帧。
enum TagFrame {
    If {
        test: String,
    },
    Foreach {
        item: String,
        collection: String,
        open: String,
        separator: String,
        close: String,
    },
}

/// 模板语言解析器。
///
/// 它维护解析状态，包括位置、节点栈和标签栈。
struct Parser<'a> {
    template: &'a str,
    pos: usize,
    nodes_stack: Vec<Vec<AstNode>>,
    tag_stack: Vec<TagFrame>,
}

impl<'a> Parser<'a> {
    fn new(template: &'a str) -> Self {
        Self {
            template,
            pos: 0,
            nodes_stack: vec![Vec::new()], // 根级节点
            tag_stack: Vec::new(),
        }
    }

    fn parse(mut self) -> Vec<AstNode> {
        while self.pos < self.template.len() {
            // 尝试优先解析结构化元素
            if self.try_parse_tag() || self.try_parse_var() {
                continue;
            }

            // 后备方案：解析为纯文本
            self.parse_text();
        }

        self.close_remaining_tags();
        self.nodes_stack.pop().unwrap_or_default()
    }

    /// 尝试解析标签：<if>, </if>, <foreach>, </foreach>, <include>。
    /// 如果成功解析并消耗了一个标签，则返回 true。
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
            if let Some(test) = extract_attr(tag_content, "test") {
                self.nodes_stack.push(Vec::new());
                self.tag_stack.push(TagFrame::If {
                    test: test.to_string(),
                });
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
            if let (Some(item), Some(collection)) = (
                extract_attr(tag_content, "item"),
                extract_attr(tag_content, "collection"),
            ) {
                let open = extract_attr(tag_content, "open").unwrap_or("");
                let separator = extract_attr(tag_content, "separator").unwrap_or(",");
                let close = extract_attr(tag_content, "close").unwrap_or("");

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
            if let Some(refid) = extract_attr(tag_content, "refid") {
                self.append_node(AstNode::Include {
                    refid: refid.to_string(),
                });
                self.pos += end_idx + 1;
                return true;
            }
        }
        false
    }

    /// 处理闭合标签 </if> 和 </foreach>
    fn handle_close_tag(&mut self, remaining: &str) -> bool {
        if remaining.starts_with("</if>")
            && let Some(TagFrame::If { .. }) = self.tag_stack.last()
            && let Some(TagFrame::If { test }) = self.tag_stack.pop()
        {
            let body = self.nodes_stack.pop().unwrap_or_default();
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
            let body = self.nodes_stack.pop().unwrap_or_default();
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

    /// 尝试解析变量表达式 #{var}
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

    /// 消耗文本直到遇到下一个特殊字符（'<' 或 '#{'）
    fn parse_text(&mut self) {
        let remaining = &self.template[self.pos..];
        let next_tag = remaining.find('<').unwrap_or(remaining.len());
        let next_var = remaining.find("#{").unwrap_or(remaining.len());
        let next_stop = std::cmp::min(next_tag, next_var);

        if next_stop > 0 {
            self.append_text(&remaining[..next_stop]);
            self.pos += next_stop;
        } else {
            // 未找到标签，或者处于可能但解析失败的标签/变量起始位置。
            // 消耗一个字符并继续。
            self.append_text(&remaining[0..1]);
            self.pos += 1;
        }
    }

    /// 辅助方法：将节点追加到当前活动作用域
    fn append_node(&mut self, node: AstNode) {
        if let Some(nodes) = self.nodes_stack.last_mut() {
            nodes.push(node);
        }
    }

    /// 辅助方法：追加文本，如果可能则与前一个文本节点合并
    fn append_text(&mut self, text: &str) {
        if let Some(nodes) = self.nodes_stack.last_mut() {
            if let Some(AstNode::Text(last_text)) = nodes.last_mut() {
                last_text.push_str(text);
            } else {
                nodes.push(AstNode::Text(text.to_string()));
            }
        }
    }

    /// 关闭模板末尾所有未闭合的标签（自动闭合行为）
    fn close_remaining_tags(&mut self) {
        while let Some(tag) = self.tag_stack.pop() {
            let body = self.nodes_stack.pop().unwrap_or_default();
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

/// 将模板字符串解析为 AST 的主要入口点。
pub fn parse_template(template: &str) -> Vec<AstNode> {
    Parser::new(template).parse()
}

/// 查找标签闭合 '>' 的索引，忽略引号内的内容。
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

/// 从标签内容中提取属性值。
/// 例如：extract_attr("test=\"abc\"", "test") -> Some("abc")
fn extract_attr<'a>(tag_content: &'a str, key: &str) -> Option<&'a str> {
    let key_len = key.len();
    for (i, _) in tag_content.match_indices(key) {
        // 确保键前有空白字符或者是字符串的开头
        if i > 0 {
            let prev = tag_content.chars().nth(i - 1).unwrap();
            if !prev.is_whitespace() {
                continue;
            }
        }

        let remaining = &tag_content[i + key_len..];
        let trimmed = remaining.trim_start();

        // 期望 '=' 后跟带引号的字符串
        if let Some(stripped) = trimmed.strip_prefix('=') {
            let after_eq = stripped.trim_start();
            if after_eq.starts_with('"')
                && let Some(end) = after_eq[1..].find('"')
            {
                // +1 跳过起始引号
                return Some(&after_eq[1..1 + end]);
            }
        }
    }
    None
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
                assert_eq!(test, "a > 1");
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
                assert_eq!(test, "x");
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
