use anyhow::{Context, Result};
use dashmap::DashMap;
use glob::glob;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock};

/// SQL 语句类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatementType {
    Select,
    Insert,
    Update,
    Delete,
    Sql,
}

impl StatementType {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "select" => Some(StatementType::Select),
            "insert" => Some(StatementType::Insert),
            "update" => Some(StatementType::Update),
            "delete" => Some(StatementType::Delete),
            "sql" => Some(StatementType::Sql),
            _ => None,
        }
    }
}

/// SQL 语句定义 (运行时对象)
/// 包含解析后的 SQL 内容及元数据
#[derive(Debug, Clone)]
pub struct SqlStatement {
    /// 语句类型 (SELECT, INSERT, etc.)
    pub r#type: StatementType,
    /// 数据库类型 (mysql, sqlite, postgres 等，可选)
    pub database_type: Option<String>,
    /// SQL 模板内容 (包含动态标签的 XML 字符串)
    pub content: Option<String>,
    /// 是否使用数据库自增主键
    pub use_generated_keys: bool,
    /// 主键列名
    pub key_column: Option<String>,
}

/// 语句存储仓库
/// 结构：Namespace -> (ID -> Vec<Arc<SqlStatement>>)
/// 使用 Vec 是为了支持同一 ID 下不同 database_type 的多态
pub type StatementStore = DashMap<String, DashMap<String, Vec<Arc<SqlStatement>>>>;

/// 全局单例存储
static STATEMENTS: OnceLock<StatementStore> = OnceLock::new();

/// 加载指定模式（glob pattern）匹配的所有 XML 映射文件
///
/// # 参数
/// * `pattern` - 文件路径匹配模式，例如 "src/resources/**/*.xml"
pub fn load(pattern: &str) -> Result<()> {
    let paths = glob(pattern).with_context(|| format!("无效的 glob 模式: {}", pattern))?;
    for entry in paths {
        let path = entry.with_context(|| format!("无法读取路径: {}", pattern))?;
        if path.is_file() {
            load_file(&path)?;
        }
    }
    Ok(())
}

/// 加载内嵌的 mapper 资源（通常用于编译进二进制的资源）
pub fn load_assets(assets: Vec<(&str, &str)>) -> Result<()> {
    for (source, content) in assets {
        parse_and_register(content, source)?;
    }
    Ok(())
}

/// 根据 SQL ID 查找对应的 SQL 语句定义
///
/// # 参数
/// * `full_id` - 完整的 SQL ID，格式为 "namespace.id"
/// * `db_type` - 数据库类型，用于筛选特定数据库的 SQL 实现
pub fn find_statement(full_id: &str, db_type: &str) -> Option<Arc<SqlStatement>> {
    let (namespace, id) = full_id.rsplit_once('.')?;

    let ns_map = STATEMENTS.get()?.get(namespace)?;
    let statements = ns_map.get(id)?;

    // 优先查找匹配 database_type 的语句，如果没有则使用默认（None）的
    let mut fallback = None;
    for stmt in statements.value().iter() {
        match stmt.database_type.as_deref() {
            Some(t) if t == db_type => return Some(stmt.clone()),
            None => fallback = Some(stmt.clone()),
            _ => {}
        }
    }

    fallback
}

/// 清理所有已加载的语句（主要用于测试环境重置状态）
pub fn clear() {
    if let Some(store) = STATEMENTS.get() {
        store.clear();
    }
}

// --- 内部实现 ---

fn load_file(path: &Path) -> Result<()> {
    let xml_content = fs::read_to_string(path)
        .with_context(|| format!("读取 Mapper 文件失败: {}", path.display()))?;
    parse_and_register(&xml_content, &path.display().to_string())
}

fn parse_and_register(xml_content: &str, source: &str) -> Result<()> {
    let (namespace, items) = parse_xml(xml_content, source)?;

    let store = STATEMENTS.get_or_init(DashMap::new);
    let ns_map = store.entry(namespace).or_default();

    for statement in items {
        let mut statements = ns_map.entry(statement.id.clone()).or_default();

        // 检查重复定义
        if statements
            .iter()
            .any(|s| s.database_type == statement.database_type)
        {
            anyhow::bail!(
                "重复的 SQL ID 定义: '{}' (Database: '{:?}', Source: '{}')",
                statement.id,
                statement.database_type,
                source
            );
        }

        statements.push(Arc::new(statement.into_sql_statement()));
    }
    Ok(())
}

struct ParsedItem {
    r#type: StatementType,
    id: String,
    database_type: Option<String>,
    use_generated_keys: bool,
    key_column: Option<String>,
    content: Option<String>,
}

impl ParsedItem {
    fn into_sql_statement(self) -> SqlStatement {
        SqlStatement {
            r#type: self.r#type,
            database_type: self.database_type,
            content: self.content,
            use_generated_keys: self.use_generated_keys,
            key_column: self.key_column,
        }
    }
}

fn parse_xml(xml: &str, source: &str) -> Result<(String, Vec<ParsedItem>)> {
    let mut reader = Reader::from_str(xml);
    // 配置 Reader 以正确处理空白和 HTML 实体（如果有必要，但这里我们主要关注标签结构）
    // trim_text(true) 会移除纯文本节点的首尾空白，但我们自己截取原始字符串时不受影响
    reader.config_mut().trim_text(true);

    let mut namespace = None;
    let mut items = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.name();
                let name_str = String::from_utf8_lossy(name.as_ref());

                if name_str == "mapper" {
                    namespace = get_attribute(e, "namespace")
                        .or_else(|| get_attribute(e, "Namespace"));
                } else if let Some(stmt_type) = StatementType::from_str(&name_str) {
                    let id = get_attribute(e, "id")
                        .ok_or_else(|| anyhow::anyhow!("SQL 语句缺少 id 属性: {}", source))?;
                    
                    let database_type = get_attribute(e, "databaseType");
                    let use_generated_keys = parse_bool(get_attribute(e, "useGeneratedKeys").as_deref());
                    let key_column = get_attribute(e, "keyColumn");

                    // 获取当前标签的结束位置作为内容的起始位置
                    let start_pos = reader.buffer_position() as usize;
                    
                    // 寻找匹配的结束标签
                    let end_pos = read_until_end_tag(&mut reader, &name_str, &mut Vec::new())?;
                    
                    // 计算内容结束位置：reader.buffer_position() 在读完结束标签后，减去结束标签长度
                    // 结束标签格式: </tag> -> 2 + tag_len + 1 (>) = 3 + tag_len
                    // 注意：quick-xml 0.3x buffer_position() 返回的是当前 buffer 的绝对偏移量
                    
                    let tag_len = name.as_ref().len();

                    if end_pos < tag_len + 3 {
                         anyhow::bail!("解析错误: 结束标签位置异常");
                    }
                    let content_end = end_pos - (tag_len + 3);
                    
                    let content = if content_end > start_pos {
                        Some(xml[start_pos..content_end].to_string())
                    } else {
                        None
                    };

                    items.push(ParsedItem {
                        r#type: stmt_type,
                        id,
                        database_type,
                        use_generated_keys,
                        key_column,
                        content,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => anyhow::bail!("XML 解析错误: {} (Source: {})", e, source),
            _ => {}
        }
        buf.clear();
    }

    let namespace = namespace.ok_or_else(|| anyhow::anyhow!("Mapper XML 缺少 namespace 属性: {}", source))?;
    Ok((namespace, items))
}

// 辅助函数：读取直到遇到指定的结束标签，返回结束标签之后的位置
fn read_until_end_tag(reader: &mut Reader<&[u8]>, target_tag: &str, buf: &mut Vec<u8>) -> Result<usize> {
    let mut depth = 0;
    loop {
        match reader.read_event_into(buf) {
            Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == target_tag.as_bytes() {
                    depth += 1;
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == target_tag.as_bytes() {
                    if depth == 0 {
                        return Ok(reader.buffer_position() as usize);
                    }
                    depth -= 1;
                }
            }
            Ok(Event::Eof) => anyhow::bail!("未找到结束标签: </{}>", target_tag),
            Err(e) => anyhow::bail!("XML 解析错误: {}", e),
            _ => {}
        }
        buf.clear();
    }
}

fn get_attribute(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .filter_map(|a| a.ok())
        .find(|a| a.key.as_ref() == key.as_bytes())
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

fn parse_bool(s: Option<&str>) -> bool {
    matches!(s.unwrap_or("").trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}
