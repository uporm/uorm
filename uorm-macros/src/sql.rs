use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    Expr, ItemFn, ItemStruct, Lit, LitStr, Meta, Result, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

// 这里把 sql_* 宏参数统一解析成同一份结构，避免在每个宏里重复写解析逻辑
struct SqlArgs {
    id: Option<String>,
    db_name: String,
}

impl Parse for SqlArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut id = None;
        let mut db_name = "default".to_string();

        if input.is_empty() {
            return Ok(SqlArgs { id, db_name });
        }

        // Try to parse a string literal first (positional id)
        if input.peek(LitStr) {
            let s: LitStr = input.parse()?;
            id = Some(s.value());

            if input.is_empty() {
                return Ok(SqlArgs { id, db_name });
            }
            input.parse::<Token![,]>()?;
        }

        // Parse named arguments
        let metas: Punctuated<Meta, Token![,]> = Punctuated::parse_terminated(input)?;
        for meta in metas {
            if let Meta::NameValue(nv) = meta
                && let Expr::Lit(expr_lit) = &nv.value
                && let Lit::Str(lit_str) = &expr_lit.lit
            {
                if nv.path.is_ident("id") {
                    id = Some(lit_str.value());
                } else if nv.path.is_ident("db_name") {
                    db_name = lit_str.value();
                }
            }
        }

        Ok(SqlArgs { id, db_name })
    }
}

pub fn sql_namespace_impl(args: TokenStream, input: TokenStream) -> TokenStream {
    let namespace = parse_macro_input!(args as LitStr).value();
    let item_struct = parse_macro_input!(input as ItemStruct);
    let struct_name = &item_struct.ident;

    let expanded = quote! {
        #item_struct

        impl #struct_name {
            pub const NAMESPACE: &'static str = #namespace;
        }
    };

    TokenStream::from(expanded)
}

fn generate_mapper_call(args: TokenStream, input: TokenStream, method_name: &str) -> TokenStream {
    let sql_args = parse_macro_input!(args as SqlArgs);
    let item_fn = parse_macro_input!(input as ItemFn);

    let fn_name = &item_fn.sig.ident;
    let fn_args = &item_fn.sig.inputs;
    let output = &item_fn.sig.output;
    let vis = &item_fn.vis;
    let block = &item_fn.block;

    // 这里强制生成 async fn，保证 exec!() 内部可以直接使用 .await
    let async_token = quote! { async };

    let id = sql_args.id.unwrap_or_else(|| fn_name.to_string());
    let db_name = sql_args.db_name;

    // Collect arguments for the struct
    let mut struct_fields = Vec::new();
    let mut field_inits = Vec::new();

    for arg in fn_args {
        if let syn::FnArg::Typed(pat_type) = arg
            && let syn::Pat::Ident(pat_ident) = &*pat_type.pat
        {
            let ident = &pat_ident.ident;
            let ty = &pat_type.ty;
            struct_fields.push(quote! { #ident: &'a #ty });
            field_inits.push(quote! { #ident: &#ident });
        }
    }
    let method_ident = syn::Ident::new(method_name, Span::call_site());
    let id_lit = LitStr::new(&id, Span::call_site());
    let db_name_lit = LitStr::new(&db_name, Span::call_site());

    let expanded = quote! {
        #vis #async_token fn #fn_name(#fn_args) #output {
            #[derive(serde::Serialize)]
            struct __UormMapperArgs<'a> {
                #(#struct_fields),*
            }
            let __uorm_args = __UormMapperArgs {
                #(#field_inits),*
            };
            let __uorm_namespace: &'static str = Self::NAMESPACE;
            let __uorm_id: &'static str = #id_lit;
            let __uorm_db_name: &'static str = #db_name_lit;

            // 在函数体内注入一个局部 exec!() 宏，确保：
            // 1) exec!() 能拿到 sql_namespace/sql_* 提供的元信息（namespace/id/db_name）
            // 2) 不会影响用户在其它位置单独使用 uorm::exec!() 的编译行为
            macro_rules! exec {
                () => {{
                    let __uorm_sql_id = format!("{}.{}", __uorm_namespace, __uorm_id);
                    let __uorm_mapper = uorm::driver_manager::UORM
                        .mapper(__uorm_db_name)
                        .expect("Database driver not found");
                    __uorm_mapper.#method_ident(&__uorm_sql_id, &__uorm_args).await
                }};
            }

            #block
        }
    };

    TokenStream::from(expanded)
}

pub fn sql_list_impl(args: TokenStream, input: TokenStream) -> TokenStream {
    generate_mapper_call(args, input, "list")
}

pub fn sql_get_impl(args: TokenStream, input: TokenStream) -> TokenStream {
    generate_mapper_call(args, input, "get")
}

pub fn sql_insert_impl(args: TokenStream, input: TokenStream) -> TokenStream {
    generate_mapper_call(args, input, "insert")
}

pub fn sql_update_impl(args: TokenStream, input: TokenStream) -> TokenStream {
    generate_mapper_call(args, input, "update")
}

pub fn sql_delete_impl(args: TokenStream, input: TokenStream) -> TokenStream {
    generate_mapper_call(args, input, "delete")
}
