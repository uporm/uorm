use glob::glob;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use syn::{LitStr, parse_macro_input};

pub fn mapper_assets_impl(input: TokenStream) -> TokenStream {
    // 1. 解析输入的字符串字面量（glob 模式）
    let pattern = parse_macro_input!(input as LitStr);
    let pattern_str = pattern.value();

    // 2. 获取 Cargo 项目的根目录
    // CARGO_MANIFEST_DIR 环境变量在编译时由 Cargo 设置，指向包含 Cargo.toml 的目录
    let manifest_dir =
        env::var("CARGO_MANIFEST_DIR").expect("编译环境异常：未设置 CARGO_MANIFEST_DIR 环境变量");
    let root = PathBuf::from(manifest_dir);

    // 3. 构建完整的 glob 模式路径
    // 将相对路径的模式拼接为绝对路径，确保 glob 查找准确
    let full_pattern = root.join(&pattern_str);
    let full_pattern_str = full_pattern.to_string_lossy();

    // 4. 查找匹配的文件
    let files: Vec<String> = match glob(&full_pattern_str) {
        Ok(paths) => paths
            .filter_map(|entry| entry.ok()) // 忽略读取错误的路径条目
            .filter(|path| path.is_file()) // 只保留文件，忽略目录
            .map(|path| path.to_string_lossy().to_string()) // 转换为字符串路径
            .collect(),
        Err(e) => {
            // 如果 glob 模式本身无效，返回编译错误
            return syn::Error::new(pattern.span(), format!("无效的 glob 模式: {}", e))
                .to_compile_error()
                .into();
        }
    };

    // 5. 生成包含文件路径和内容的元组代码片段
    // 使用 include_str! 宏在编译时加载文件内容，确保运行时无需读取文件系统
    let assets: Vec<_> = files
        .iter()
        .map(|f| {
            quote! {
                (#f, include_str!(#f))
            }
        })
        .collect();

    // 6. 基于模式字符串生成唯一的哈希值
    // 用于生成唯一的函数名，防止在同一作用域多次调用宏（即针对不同模式）时产生命名冲突
    let mut hasher = DefaultHasher::new();
    pattern_str.hash(&mut hasher);
    let hash = hasher.finish();

    // 生成唯一的注册函数名，例如：__uorm_auto_register_assets_123456789
    let fn_name = format_ident!("__uorm_auto_register_assets_{}", hash);

    // 7. 生成最终的代码
    // 使用 #[uorm::ctor::ctor] 属性宏，使该函数在程序启动（main 函数之前）自动执行
    let output = quote! {
        #[uorm::ctor::ctor]
        fn #fn_name() {
            // 将所有资源文件路径和内容收集到向量中
            let assets = vec![
                #(#assets),*
            ];

            // 调用运行时加载器注册资源
            // 使用 let _ = ... 忽略返回值，因为这是在初始化阶段，若失败通常通过日志记录
            let _ = uorm::mapper_loader::load_assets(assets);
        }
    };

    output.into()
}
