use anyhow::{Context, Result};
use dashmap::DashMap;
use glob::glob;
use quick_xml::Writer;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::{Arc, OnceLock};

/// SQL 映射对象，包含 SQL 内容及相关配置
#[derive(Debug, Clone)]
pub struct SqlMapper {
    /// 数据库类型
    pub database_type: Option<String>,
    /// SQL 文本内容
    pub content: Option<String>,
    /// 是否使用数据库自增主键
    pub use_generated_keys: bool,
    /// 主键列名
    pub key_column: Option<String>,
}

/// SQL 映射器存储仓库，使用 DashMap 实现并发安全的存储
/// 结构：Namespace -> (ID -> Vec<Arc<SqlMapper>>)
pub type SqlMapperStore = DashMap<String, DashMap<String, Vec<Arc<SqlMapper>>>>;

/// 全局单例的 SQL 映射器存储
static SQL_MAPPERS: OnceLock<SqlMapperStore> = OnceLock::new();

/// 资源提供者特征，用于抽象资源加载
pub trait AssetProvider {
    fn list(&self) -> Vec<&[u8]>;
}

/// SQL配置项结构，对应 XML 中的具体标签
#[derive(Debug)]
pub struct SqlItem {
    /// SQL 语句唯一标识
    pub id: String,
    /// 数据库类型
    pub database_type: Option<String>,
    /// 是否使用自增主键配置字符串
    pub use_generated_keys: Option<String>,
    /// 主键列名配置
    pub key_column: Option<String>,
    /// SQL 文本内容
    pub content: Option<String>,
}

impl From<&SqlItem> for SqlMapper {
    fn from(item: &SqlItem) -> Self {
        let use_generated_keys = parse_truthy(item.use_generated_keys.as_deref());

        Self {
            database_type: item.database_type.clone(),
            content: item.content.clone(),
            use_generated_keys,
            key_column: item.key_column.clone(),
        }
    }
}

fn parse_truthy(s: Option<&str>) -> bool {
    let Some(s) = s else {
        return false;
    };
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes")
}

/// 加载指定模式（glob pattern）匹配的所有 XML 映射文件
///
/// # 参数
/// * `pattern` - 文件路径匹配模式，例如 "src/resources/**/*.xml"
///
/// # 返回
/// * `Result<()>` - 加载成功返回 Ok(())，否则返回错误
pub fn load(pattern: &str) -> Result<()> {
    let paths = glob(pattern).with_context(|| format!("读取 glob 模式失败: {}", pattern))?;
    for entry in paths {
        let path = entry.with_context(|| format!("读取路径失败: {}", pattern))?;
        if path.is_file() {
            process_mapper_file(&path)?;
        }
    }
    Ok(())
}

/// 加载内嵌的 mapper 资源（通常用于编译进二进制的资源）
pub fn load_assets(assets: Vec<(&str, &str)>) -> Result<()> {
    for (source, content) in assets {
        process_mapper_data(content, source)?;
    }
    Ok(())
}

/// 根据 SQL ID 查找对应的 Mapper 配置
///
/// # 参数
/// * `sql_id` - 完整的 SQL ID，格式为 "namespace.id"
/// * `db_type` - 数据库类型，例如 "mysql", "postgres"
pub fn find_mapper(sql_id: &str, db_type: &str) -> Option<Arc<SqlMapper>> {
    let (namespace, id) = sql_id.rsplit_once('.')?;

    let store = SQL_MAPPERS.get()?;
    let ns_map = store.get(namespace)?;
    let mappers = ns_map.get(id)?;

    let mut fallback = None;
    for mapper in mappers.value().iter() {
        match mapper.database_type.as_deref() {
            Some(t) if t == db_type => return Some(mapper.clone()),
            None => fallback = Some(mapper.clone()),
            _ => {}
        }
    }

    fallback
}

/// 处理单个 Mapper 文件
fn process_mapper_file(path: &Path) -> Result<()> {
    let xml_content =
        fs::read_to_string(path).with_context(|| format!("读取文件失败: {}", path.display()))?;
    let source = path.display().to_string();
    process_mapper_data(&xml_content, &source)
}

fn get_attr(e: &BytesStart<'_>, key: &[u8]) -> Result<Option<String>> {
    for attr in e.attributes() {
        let attr = attr?;
        if attr.key.as_ref() == key {
            return Ok(Some(attr.unescape_value()?.into_owned()));
        }
    }
    Ok(None)
}

fn get_first_attr(e: &BytesStart<'_>, keys: &[&[u8]]) -> Result<Option<String>> {
    for &key in keys {
        if let Some(v) = get_attr(e, key)? {
            return Ok(Some(v));
        }
    }
    Ok(None)
}

fn required_attr(e: &BytesStart<'_>, key: &[u8], source: &str) -> Result<String> {
    get_attr(e, key)?.with_context(|| {
        format!(
            "XML 节点缺少 {} 属性: {}",
            String::from_utf8_lossy(key),
            source
        )
    })
}

fn is_sql_node(name: &[u8]) -> bool {
    matches!(name, b"sql" | b"select" | b"insert" | b"update" | b"delete")
}

fn read_inner_xml(
    reader: &mut Reader<&[u8]>,
    end_name: quick_xml::name::QName<'_>,
) -> Result<String> {
    let mut buf = Vec::new();
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let mut depth: usize = 0;

    loop {
        buf.clear();
        let event = reader.read_event_into(&mut buf)?;
        match event {
            Event::Start(e) => {
                depth += 1;
                writer.write_event(Event::Start(e.to_owned()))?;
            }
            Event::Empty(e) => {
                writer.write_event(Event::Empty(e.to_owned()))?;
            }
            Event::End(e) => {
                if depth == 0 && e.name() == end_name {
                    break;
                }
                depth = depth.saturating_sub(1);
                writer.write_event(Event::End(e.to_owned()))?;
            }
            Event::Text(e) => {
                writer.write_event(Event::Text(e.to_owned()))?;
            }
            Event::CData(e) => {
                writer.write_event(Event::CData(e.to_owned()))?;
            }
            Event::Comment(e) => {
                writer.write_event(Event::Comment(e.to_owned()))?;
            }
            Event::Eof => anyhow::bail!("Unexpected EOF while reading inner XML"),
            _ => {}
        }
    }

    let bytes = writer.into_inner().into_inner();
    Ok(String::from_utf8(bytes)?)
}

fn parse_sql_item_start(
    reader: &mut Reader<&[u8]>,
    e: BytesStart<'_>,
    source: &str,
) -> Result<SqlItem> {
    let id = required_attr(&e, b"id", source)?;
    let database_type = get_attr(&e, b"databaseType")?;
    let use_generated_keys = get_attr(&e, b"useGeneratedKeys")?;
    let key_column = get_attr(&e, b"keyColumn")?;

    let content = read_inner_xml(reader, e.name())
        .with_context(|| format!("读取 SQL 节点内容失败: {}", source))?;

    Ok(SqlItem {
        id,
        database_type,
        use_generated_keys,
        key_column,
        content: Some(content),
    })
}

fn parse_sql_item_empty(e: BytesStart<'_>, source: &str) -> Result<SqlItem> {
    let id = required_attr(&e, b"id", source)?;
    let database_type = get_attr(&e, b"databaseType")?;
    let use_generated_keys = get_attr(&e, b"useGeneratedKeys")?;
    let key_column = get_attr(&e, b"keyColumn")?;

    Ok(SqlItem {
        id,
        database_type,
        use_generated_keys,
        key_column,
        content: Some(String::new()),
    })
}

fn parse_mapper_xml(xml_content: &str, source: &str) -> Result<(String, Vec<SqlItem>)> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut namespace: Option<String> = None;
    let mut items: Vec<SqlItem> = Vec::new();

    loop {
        buf.clear();
        let event = reader.read_event_into(&mut buf)?;

        match event {
            Event::Start(e) => {
                let name = e.name();
                let name_bytes = name.as_ref();
                let is_mapper = name_bytes == b"mapper";
                let is_sql = is_sql_node(name_bytes);

                if is_mapper {
                    namespace = get_first_attr(&e, &[b"namespace", b"Namespace"])?;
                    if namespace.is_none() {
                        anyhow::bail!("XML mapper 缺少 namespace 属性: {}", source);
                    }
                    continue;
                }

                if is_sql {
                    items.push(parse_sql_item_start(&mut reader, e, source)?);
                    continue;
                }

                reader.read_to_end_into(e.name(), &mut Vec::new())?;
            }
            Event::Empty(e) => {
                if is_sql_node(e.name().as_ref()) {
                    items.push(parse_sql_item_empty(e, source)?);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    let namespace =
        namespace.ok_or_else(|| anyhow::anyhow!("XML mapper 缺少 <mapper> 根节点: {}", source))?;
    Ok((namespace, items))
}

/// 解析 Mapper XML 内容并存入全局存储
fn process_mapper_data(xml_content: &str, source: &str) -> Result<()> {
    let (namespace, items) = parse_mapper_xml(xml_content, source)
        .with_context(|| format!("XML 解析失败: {}", source))?;

    let store = SQL_MAPPERS.get_or_init(DashMap::new);
    let ns_map = store.entry(namespace.clone()).or_default();

    for item in items {
        let sql_mapper = SqlMapper::from(&item);
        let mut mappers = ns_map.entry(item.id.clone()).or_default();
        if mappers
            .iter()
            .any(|existing| existing.database_type == sql_mapper.database_type)
        {
            anyhow::bail!(
                "文件 '{}' 中发现重复的 ID: '{}' (命名空间: '{}', databaseType: '{:?}')",
                source,
                item.id,
                namespace,
                sql_mapper.database_type
            );
        }

        mappers.push(Arc::new(sql_mapper));
    }
    Ok(())
}

/// 清理所有已加载的 mapper（主要用于测试环境重置状态）
pub fn clear_mappers() {
    if let Some(store) = SQL_MAPPERS.get() {
        store.clear();
    }
}
