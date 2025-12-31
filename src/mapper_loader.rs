use crate::Result;
use crate::error::DbError;
use crate::tpl::cache;
use dashmap::DashMap;
use glob::glob;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock};

/// SQL statement type.
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

/// A SQL statement definition (runtime representation).
///
/// Holds the parsed SQL template (raw XML inner text) plus metadata.
#[derive(Debug, Clone)]
pub struct SqlStatement {
    /// Statement type (SELECT, INSERT, etc.).
    pub r#type: StatementType,
    /// Database type (mysql, sqlite, postgres, etc.). Optional.
    pub database_type: Option<String>,
    /// SQL template content (may contain dynamic XML tags).
    pub content: Option<String>,
    /// Whether to use database-generated keys.
    pub use_generated_keys: bool,
    /// Primary key column name.
    pub key_column: Option<String>,
}

/// Statement repository.
///
/// Layout: namespace -> (id -> Vec<Arc<SqlStatement>>).
/// A Vec allows multiple variants under the same id, distinguished by `database_type`.
pub type StatementStore = DashMap<String, DashMap<String, Vec<Arc<SqlStatement>>>>;

/// Global singleton storage.
static STATEMENTS: OnceLock<StatementStore> = OnceLock::new();

/// Load all XML mapper files matched by the given glob pattern.
///
/// # Parameters
/// * `pattern` - File path glob pattern, e.g. `"src/resources/**/*.xml"`.
pub fn load(pattern: &str) -> Result<()> {
    let paths = glob(pattern)
        .map_err(|e| DbError::MapperLoadError(format!("无效的 glob 模式: {} - {}", pattern, e)))?;
    for entry in paths {
        let path: std::path::PathBuf = entry.map_err(|e: glob::GlobError| {
            DbError::MapperLoadError(format!("无法读取路径: {} - {}", pattern, e))
        })?;
        if path.is_file() {
            load_file(&path)?;
        }
    }
    Ok(())
}

/// Load embedded mapper assets (typically compiled into the binary).
pub fn load_assets(assets: Vec<(&str, &str)>) -> Result<()> {
    for (source, content) in assets {
        parse_and_register(content, source)?;
    }
    Ok(())
}

/// Find a SQL statement definition by SQL id.
///
/// # Parameters
/// * `full_id` - Full SQL id in the form `"namespace.id"`.
/// * `db_type` - Database type used to pick a DB-specific implementation.
pub fn find_statement(full_id: &str, db_type: &str) -> Option<Arc<SqlStatement>> {
    let (namespace, id) = full_id.rsplit_once('.')?;

    let ns_map = STATEMENTS.get()?.get(namespace)?;
    let statements = ns_map.get(id)?;

    // Prefer an entry that matches `database_type`; fall back to the default (`None`) entry.
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

/// Clear all loaded statements (mainly to reset state in tests).
pub fn clear() {
    if let Some(store) = STATEMENTS.get() {
        store.clear();
    }
}

// --- Internal implementation ---

fn load_file(path: &Path) -> Result<()> {
    let xml_content = fs::read_to_string(path).map_err(|e| {
        DbError::MapperLoadError(format!(
            "读取 Mapper 文件失败: {} (cause: {})",
            path.display(),
            e
        ))
    })?;
    parse_and_register(&xml_content, &path.display().to_string())
}

fn parse_and_register(xml_content: &str, source: &str) -> Result<()> {
    let (namespace, items) = parse_xml(xml_content, source)?;

    let store = STATEMENTS.get_or_init(DashMap::new);
    let ns_map = store.entry(namespace.clone()).or_default();

    for mut statement in items {
        if let Some(content) = &mut statement.content {
            *content = content.trim().to_string();
        }

        // Register in template cache for <include> tags.
        if let Some(content) = &statement.content {
            let full_id = format!("{}.{}", namespace, statement.id);
            cache::get_ast(&full_id, content);
        }

        let mut statements = ns_map.entry(statement.id.clone()).or_default();

        // Reject duplicate definitions.
        if statements
            .iter()
            .any(|s| s.database_type == statement.database_type)
        {
            return Err(DbError::MapperLoadError(format!(
                "重复的 SQL ID 定义: '{}' (Database: '{:?}', Source: '{}')",
                statement.id, statement.database_type, source
            )));
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
    // Configure the reader. We trim text nodes to simplify parsing; buffer-position slicing is
    // based on the original XML and is not affected by trimming.
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
                    namespace =
                        get_attribute(e, "namespace").or_else(|| get_attribute(e, "Namespace"));
                } else if let Some(stmt_type) = StatementType::from_str(&name_str) {
                    let id = get_attribute(e, "id").ok_or_else(|| {
                        DbError::MapperLoadError(format!("SQL 语句缺少 id 属性: {}", source))
                    })?;

                    let database_type = get_attribute(e, "databaseType");
                    let use_generated_keys =
                        parse_bool(get_attribute(e, "useGeneratedKeys").as_deref());
                    let key_column = get_attribute(e, "keyColumn");

                    // Use the end of the start tag as the content start position.
                    let start_pos = reader.buffer_position() as usize;

                    // Read until we reach the matching end tag.
                    let end_pos = read_until_end_tag(&mut reader, &name_str, &mut Vec::new())?;

                    // Compute the content end position.
                    // After reading the end tag, `buffer_position()` points to the position right
                    // after it. The end tag format is `</tag>` -> 3 + tag_len bytes.
                    // Note: in quick-xml 0.3x, `buffer_position()` is an absolute offset.

                    let tag_len = name.as_ref().len();

                    if end_pos < tag_len + 3 {
                        return Err(DbError::MapperLoadError(
                            "解析错误: 结束标签位置异常".to_string(),
                        ));
                    }
                    let content_end = end_pos - (tag_len + 3);

                    let content = if content_end > start_pos {
                        let raw_content = &xml[start_pos..content_end];
                        // Unescape XML entities like &lt;, &gt;, &amp;, etc.
                        // The template parser expects raw characters.
                        quick_xml::escape::unescape(raw_content)
                            .map(|s| s.into_owned())
                            .ok()
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
            Err(e) => {
                return Err(DbError::MapperLoadError(format!(
                    "XML 解析错误: {} (Source: {})",
                    e, source
                )));
            }
            _ => {}
        }
        buf.clear();
    }

    let namespace = namespace.ok_or_else(|| {
        DbError::MapperLoadError(format!("Mapper XML 缺少 namespace 属性: {}", source))
    })?;
    Ok((namespace, items))
}

// Helper: read until the matching end tag is found, and return the position right after it.
fn read_until_end_tag(
    reader: &mut Reader<&[u8]>,
    target_tag: &str,
    buf: &mut Vec<u8>,
) -> Result<usize> {
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
            Ok(Event::Eof) => {
                return Err(DbError::MapperLoadError(format!(
                    "未找到结束标签: </{}>",
                    target_tag
                )));
            }
            Err(e) => return Err(DbError::MapperLoadError(format!("XML 解析错误: {}", e))),
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
    matches!(
        s.unwrap_or("").trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
