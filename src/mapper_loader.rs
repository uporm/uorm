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

/// SQL 语句类型。
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

/// SQL 语句定义（运行时表示）。
///
/// 保存解析后的 SQL 模板（XML 内部原文）及元数据。
#[derive(Debug, Clone)]
pub struct SqlStatement {
    /// 语句类型（SELECT、INSERT 等）。
    pub r#type: StatementType,
    /// 数据库类型（mysql、sqlite、postgres 等），可选。
    pub database_type: Option<String>,
    /// SQL 模板内容（可能包含动态 XML 标签）。
    pub content: Option<String>,
    /// 是否返回生成的主键。
    pub return_key: bool,
}

/// 语句仓库。
///
/// 结构：namespace -> (id -> Vec<Arc<SqlStatement>>)。
/// Vec 允许同一 id 下存在多种变体，通过 `database_type` 区分。
pub type StatementStore = DashMap<String, DashMap<String, Vec<Arc<SqlStatement>>>>;

/// 全局单例存储。
static STATEMENTS: OnceLock<StatementStore> = OnceLock::new();

/// 按给定 glob 模式加载所有 XML mapper 文件。
///
/// # 参数
/// * `pattern` - 文件路径 glob 模式，如 `"src/resources/**/*.xml"`。
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

/// 加载内嵌的 mapper 资源（通常编译进二进制）。
pub fn load_assets(assets: Vec<(&str, &str)>) -> Result<()> {
    for (source, content) in assets {
        parse_and_register(content, source)?;
    }
    Ok(())
}

/// 通过 SQL id 查找 SQL 语句定义。
///
/// # 参数
/// * `full_id` - 完整 SQL id，形如 `"namespace.id"`。
/// * `db_type` - 数据库类型，用于选择特定实现。
pub fn find_statement(full_id: &str, db_type: &str) -> Option<Arc<SqlStatement>> {
    let (namespace, id) = full_id.rsplit_once('.')?;

    let ns_map = STATEMENTS.get()?.get(namespace)?;
    let statements = ns_map.get(id)?;

    // 优先匹配 `database_type`，否则回退到默认（`None`）项
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

/// 清空所有已加载语句（主要用于测试重置状态）。
pub fn clear() {
    if let Some(store) = STATEMENTS.get() {
        store.clear();
    }
}

// --- 内部实现 ---

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

        // 注册到模板缓存以支持 <include> 标签
        if let Some(content) = &statement.content {
            let full_id = format!("{}.{}", namespace, statement.id);
            cache::get_ast(&full_id, content);
        }

        let mut statements = ns_map.entry(statement.id.clone()).or_default();

        // 拒绝重复定义
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
    return_key: bool,
    content: Option<String>,
}

impl ParsedItem {
    fn into_sql_statement(self) -> SqlStatement {
        SqlStatement {
            r#type: self.r#type,
            database_type: self.database_type,
            content: self.content,
            return_key: self.return_key,
        }
    }
}

fn parse_xml(xml: &str, source: &str) -> Result<(String, Vec<ParsedItem>)> {
    let mut reader = Reader::from_str(xml);
    // 配置 reader。裁剪文本节点以简化解析；buffer_position 基于原始 XML，不受裁剪影响。
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
                    let return_key = parse_bool(get_attribute(e, "returnKey").as_deref());

                    // 以起始标签末尾作为内容开始位置。
                    let start_pos = reader.buffer_position() as usize;

                    // 读取直到匹配到结束标签。
                    let end_pos = read_until_end_tag(&mut reader, &name_str, &mut Vec::new())?;

                    // 计算内容结束位置。
                    // 读取结束标签后，`buffer_position()` 指向其后的位置。
                    // 结束标签格式为 `</tag>` -> 3 + tag_len 字节。
                    // 注意：quick-xml 0.3x 中 `buffer_position()` 是绝对偏移。

                    let tag_len = name.as_ref().len();

                    if end_pos < tag_len + 3 {
                        return Err(DbError::MapperLoadError(
                            "解析错误: 结束标签位置异常".to_string(),
                        ));
                    }
                    let content_end = end_pos - (tag_len + 3);

                    let content = if content_end > start_pos {
                        let raw_content = &xml[start_pos..content_end];
                        // 反转义 XML 实体，如 &lt;、&gt;、&amp; 等。
                        // 模板解析器需要原始字符。
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
                        return_key,
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

// 辅助：读取直到匹配的结束标签，并返回其后的位置。
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
