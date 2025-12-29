use glob::glob;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use syn::{LitStr, parse_macro_input};

pub fn mapper_assets_impl(input: TokenStream) -> TokenStream {
    let dir_lit = parse_macro_input!(input as LitStr);
    let dir = dir_lit.value();

    // 1. Resolve directory path relative to CARGO_MANIFEST_DIR
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let root = PathBuf::from(manifest_dir);
    let dir_path = root.join(&dir);

    // 2. Validate directory existence
    if !dir_path.exists() || !dir_path.is_dir() {
        return syn::Error::new(
            dir_lit.span(),
            format!("Directory not found: {}", dir_path.display()),
        )
        .to_compile_error()
        .into();
    }

    // 3. Find all XML files recursively
    let pattern = dir_path.join("**/*.xml");
    let pattern_str = pattern.to_string_lossy();

    let paths = match glob(&pattern_str) {
        Ok(paths) => paths,
        Err(e) => {
            return syn::Error::new(dir_lit.span(), format!("Invalid glob pattern: {}", e))
                .to_compile_error()
                .into();
        }
    };

    // 4. Generate asset loading code
    let assets: Vec<_> = paths
        .filter_map(Result::ok)
        .filter(|path| path.is_file())
        .filter_map(|path| {
            let abs_path = path.canonicalize().ok()?;
            let abs_path_str = abs_path.to_string_lossy().to_string();
            // Use include_bytes! for binary embedding and runtime string conversion
            Some(quote! {
                (#abs_path_str, std::str::from_utf8(include_bytes!(#abs_path_str)).expect("Invalid UTF-8 in mapper XML"))
            })
        })
        .collect();

    // 5. Generate unique registration function
    let mut hasher = DefaultHasher::new();
    dir.hash(&mut hasher);
    let fn_name = format_ident!("__uorm_auto_register_assets_{}", hasher.finish());

    let pattern_lit = LitStr::new(&pattern_str, proc_macro2::Span::call_site());

    quote! {
        #[uorm::ctor::ctor(crate_path = ::uorm::ctor)]
        fn #fn_name() {
            #[cfg(debug_assertions)]
            {
                uorm::mapper_loader::load(#pattern_lit).expect("Failed to load mapper assets from disk");
            }
            #[cfg(not(debug_assertions))]
            {
                let assets = vec![
                    #(#assets),*
                ];
                uorm::mapper_loader::load_assets(assets).expect("Failed to load mapper assets");
            }
        }
    }
    .into()
}
