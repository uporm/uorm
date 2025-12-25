use crate::tpl::ast::{AstNode, Expr, Op};
use crate::udbc::value::Value;
use std::collections::HashMap;

/// Represents a stack frame during template parsing to handle nested tags.
///
/// When a start tag (like `<if>`) is encountered, a new `TagFrame` is pushed onto the stack.
/// This allows the parser to keep track of the current tag's attributes and nesting level.
enum TagFrame {
    /// An `<if>` tag frame, storing the test expression.
    If { test: Expr },
    /// A `<foreach>` tag frame, storing the iteration details.
    Foreach {
        item: String,
        collection: String,
        open: String,
        separator: String,
        close: String,
    },
}

/// A hand-written recursive-descent style parser for the SQL template language.
///
/// It supports:
/// - Plain text (SQL)
/// - Variable interpolation: `#{var}`
/// - Conditional logic: `<if test="...">...</if>`
/// - Iteration: `<foreach item="..." collection="..." ...>...</foreach>`
/// - Template inclusion: `<include refid="..." />`
///
/// The parser uses a stack-based approach to handle nested tags correctly.
struct Parser<'a> {
    /// The original template string being parsed.
    template: &'a str,
    /// Current character position in the template.
    pos: usize,
    /// A stack of node collections. Each level corresponds to the children of a nested tag.
    /// The first element is always the root-level nodes.
    nodes_stack: Vec<Vec<AstNode>>,
    /// A stack of active tags being parsed.
    tag_stack: Vec<TagFrame>,
}

impl<'a> Parser<'a> {
    /// Creates a new parser instance for the given template string.
    fn new(template: &'a str) -> Self {
        Self {
            template,
            pos: 0,
            nodes_stack: vec![Vec::new()], // Initialize with root level.
            tag_stack: Vec::new(),
        }
    }

    /// Parses the entire template and returns a list of root-level `AstNode`s.
    fn parse(mut self) -> Vec<AstNode> {
        while self.pos < self.template.len() {
            // Try to parse structured elements (tags or variables) first.
            if self.try_parse_tag() || self.try_parse_var() {
                continue;
            }

            // Fallback: parse as plain text if no structured elements match at current position.
            self.parse_text();
        }

        // Close any tags that were left open (e.g., missing </if>).
        self.close_remaining_tags();

        // Return the root-level nodes.
        self.nodes_stack.pop().unwrap_or_default()
    }

    /// Try to parse a tag: `<if>`, `</if>`, `<foreach>`, `</foreach>`, `<include>`.
    /// Returns true if a tag was successfully parsed and consumed.
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

    /// Handle <if test="...">
    fn handle_if_tag(&mut self, remaining: &str) -> bool {
        if let Some(end_idx) = find_tag_end(remaining) {
            let tag_content = &remaining[4..end_idx]; // Skip "<if "
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

    /// Handle <foreach item="..." collection="...">
    fn handle_foreach_tag(&mut self, remaining: &str) -> bool {
        if let Some(end_idx) = find_tag_end(remaining) {
            let tag_content = &remaining[9..end_idx]; // Skip "<foreach "
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

    /// Handle <include refid="..." />
    fn handle_include_tag(&mut self, remaining: &str) -> bool {
        if let Some(end_idx) = find_tag_end(remaining) {
            let tag_content = &remaining[8..end_idx]; // Skip "<include"
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

    /// Handle closing tags `</if>` and `</foreach>`.
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

            // Only trim if the whitespace contains a newline (block formatting).
            // If it's just spaces (inline formatting), preserve it.
            if whitespace.contains('\n') {
                if trimmed.is_empty() {
                    nodes.remove(0);
                } else {
                    *text = trimmed.to_string();
                }
            }
        }

        // After potential removal, check last (which might be the same node if len=1)
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

    /// Try to parse a variable expression: `#{var}`.
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

    /// Consume text until the next special sequence (`'<'` or `"#{"`).
    fn parse_text(&mut self) {
        let remaining = &self.template[self.pos..];
        let next_tag = remaining.find('<').unwrap_or(remaining.len());
        let next_var = remaining.find("#{").unwrap_or(remaining.len());
        let next_stop = std::cmp::min(next_tag, next_var);

        if next_stop > 0 {
            self.append_text(&remaining[..next_stop]);
            self.pos += next_stop;
        } else {
            // Either no tag was found, or we're at a tag/var boundary that didn't parse.
            // Consume one character to make progress and avoid infinite loops.
            self.append_text(&remaining[0..1]);
            self.pos += 1;
        }
    }

    /// Append a node to the current active scope.
    fn append_node(&mut self, node: AstNode) {
        if let Some(nodes) = self.nodes_stack.last_mut() {
            nodes.push(node);
        }
    }

    /// Append text, merging with the previous text node when possible.
    fn append_text(&mut self, text: &str) {
        if let Some(nodes) = self.nodes_stack.last_mut() {
            if let Some(AstNode::Text(last_text)) = nodes.last_mut() {
                last_text.push_str(text);
            } else {
                nodes.push(AstNode::Text(text.to_string()));
            }
        }
    }

    /// Auto-close any remaining unclosed tags at the end of the template.
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

/// Main entry point: parse a template string into an AST.
pub fn parse_template(template: &str) -> Vec<AstNode> {
    Parser::new(template).parse()
}

/// Find the index of the closing `>` for a tag, ignoring quoted content.
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

/// Parse attributes from tag content into a HashMap
fn parse_attributes(content: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();

    let mut rest = content;
    while !rest.is_empty() {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }

        // Find key end
        let key_end = rest
            .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .unwrap_or(rest.len());
        if key_end == 0 {
            // Should not happen if trim_start worked and we have valid chars.
            // Consume one char to avoid infinite loop if garbage present
            rest = &rest[1..];
            continue;
        }
        let key = &rest[..key_end];
        rest = rest[key_end..].trim_start();

        // Expect '='
        if !rest.starts_with('=') {
            continue;
        }
        rest = rest[1..].trim_start();

        // Expect quote
        if rest.is_empty() {
            break;
        }
        let quote = rest.chars().next().unwrap();
        if quote != '"' && quote != '\'' {
            continue;
        }
        rest = &rest[1..];

        // Find matching quote
        if let Some(val_end) = rest.find(quote) {
            let val = &rest[..val_end];
            attrs.insert(key.to_string(), val.to_string());
            rest = &rest[val_end + 1..];
        } else {
            break; // Unclosed quote
        }
    }
    attrs
}

fn parse_expr(input: &str) -> Expr {
    // 1. Split by OR
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
    // Check operators. Order matters (longest first).
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

    // Implicit boolean check
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
    // Variable
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
