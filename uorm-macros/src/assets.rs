use glob::glob;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use syn::{LitStr, parse_macro_input};

pub fn mapper_assets_impl(input: TokenStream) -> TokenStream {
    // 1) Parse the input string literal (glob pattern).
    let pattern = parse_macro_input!(input as LitStr);
    let pattern_str = pattern.value();

    // 2) Get the crate root directory.
    // The CARGO_MANIFEST_DIR env var is set by Cargo at compile time and points to the directory
    // containing Cargo.toml.
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .expect("Compilation environment error: CARGO_MANIFEST_DIR environment variable not set");
    let root = PathBuf::from(manifest_dir);

    // 3) Build the full glob pattern path.
    // Join relative patterns with the crate root so glob matching is reliable.
    let full_pattern = root.join(&pattern_str);
    let full_pattern_str = full_pattern.to_string_lossy();

    // 4) Find matching files.
    let files: Vec<String> = match glob(&full_pattern_str) {
        Ok(paths) => paths
            .filter_map(|entry| entry.ok()) // Ignore entries we failed to read.
            .filter(|path| path.is_file()) // Keep files only; ignore directories.
            .map(|path| path.to_string_lossy().to_string()) // Convert to a string path.
            .collect(),
        Err(e) => {
            // If the glob pattern itself is invalid, return a compilation error
            return syn::Error::new(pattern.span(), format!("Invalid glob pattern: {}", e))
                .to_compile_error()
                .into();
        }
    };

    // 5) Generate tuples of (path, content).
    // `include_str!` embeds file contents at compile time so runtime does not touch the filesystem.
    let assets: Vec<_> = files
        .iter()
        .map(|f| {
            quote! {
                (#f, include_str!(#f))
            }
        })
        .collect();

    // 6. Generate a unique hash value based on the pattern string
    // Used to generate a unique function name to prevent naming conflicts when calling the macro multiple times in the same scope (i.e., for different patterns)
    let mut hasher = DefaultHasher::new();
    pattern_str.hash(&mut hasher);
    let hash = hasher.finish();

    // Generate a unique registration function name, e.g. __uorm_auto_register_assets_123456789.
    let fn_name = format_ident!("__uorm_auto_register_assets_{}", hash);

    // 7) Generate the final code.
    // `#[uorm::ctor::ctor]` runs this function at startup (before `main`).
    let output = quote! {
        #[uorm::ctor::ctor]
        fn #fn_name() {
            // Collect all asset file paths and contents into a Vec.
            let assets = vec![
                #(#assets),*
            ];

            // Register assets via the runtime loader.
            // Ignore the result because this runs during initialization and failures are typically
            // reported via logging.
            let _ = uorm::mapper_loader::load_assets(assets);
        }
    };

    output.into()
}
