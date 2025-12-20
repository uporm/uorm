use crate::tpl::AstNode;
use crate::tpl::parser::parse_template;
use dashmap::DashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock};

#[derive(Clone)]
pub struct CachedTemplate {
    pub ast: Arc<Vec<AstNode>>,
    pub content_hash: u64,
}

/// 缓存模板 AST
pub(crate) static TEMPLATE_CACHE: LazyLock<DashMap<String, CachedTemplate>> =
    LazyLock::new(DashMap::new);

pub(crate) fn get_ast(template_name: &str, template_content: &str) -> Arc<Vec<AstNode>> {
    let mut hasher = DefaultHasher::new();
    template_content.hash(&mut hasher);
    let new_hash = hasher.finish();

    if let Some(cached) = TEMPLATE_CACHE.get(template_name) {
        if cached.content_hash == new_hash {
            return cached.ast.clone();
        }
    }

    let ast = Arc::new(parse_template(template_content));
    TEMPLATE_CACHE.insert(
        template_name.to_string(),
        CachedTemplate {
            ast: ast.clone(),
            content_hash: new_hash,
        },
    );
    ast
}
