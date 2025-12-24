use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    Expr, ItemFn, ItemStruct, Lit, LitStr, Meta, Result, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

/// Arguments for the `#[sql]` attribute macro.
///
/// Supports both positional and named arguments:
/// - Positional: `#[sql("my_id")]` or `#[sql("my_namespace")]`
/// - Named: `#[sql(id = "my_id", database = "other_db", namespace = "my_ns")]`
struct SqlArgs {
    /// The first positional string literal, which can represent either an ID (on functions)
    /// or a namespace (on structs).
    value: Option<String>,
    /// Explicitly provided SQL ID.
    id: Option<String>,
    /// The name of the database driver to use (defaults to "default").
    database: Option<String>,
    /// The XML namespace where the SQL is defined.
    namespace: Option<String>,
}

impl Parse for SqlArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut value = None;
        let mut id = None;
        let mut database = None;
        let mut namespace = None;

        if input.is_empty() {
            return Ok(SqlArgs { value, id, database, namespace });
        }

        // Try to parse an optional positional string literal first.
        if input.peek(LitStr) {
            let s: LitStr = input.parse()?;
            value = Some(s.value());

            if input.is_empty() {
                return Ok(SqlArgs { value, id, database, namespace });
            }
            // If more arguments follow, they must be separated by a comma.
            input.parse::<Token![,]>()?;
        }

        // Parse remaining named arguments like `id = "..."`.
        let metas: Punctuated<Meta, Token![,]> = Punctuated::parse_terminated(input)?;
        for meta in metas {
            if let Meta::NameValue(nv) = meta
                && let Expr::Lit(expr_lit) = &nv.value
                && let Lit::Str(lit_str) = &expr_lit.lit
            {
                if nv.path.is_ident("id") {
                    id = Some(lit_str.value());
                } else if nv.path.is_ident("database") {
                    database = Some(lit_str.value());
                } else if nv.path.is_ident("namespace") {
                    namespace = Some(lit_str.value());
                }
            }
        }

        Ok(SqlArgs { value, id, database, namespace })
    }
}

/// The entry point for the `#[sql]` attribute macro.
///
/// This macro can be applied to:
/// 1. A struct: to define the default SQL namespace for all methods in its impl block.
/// 2. A function: to bind the function to a specific SQL statement in a Mapper XML.
pub fn sql_impl(args: TokenStream, input: TokenStream) -> TokenStream {
    let input_clone = input.clone();
    // Dispatch based on whether the attribute is applied to a struct or a function.
    if syn::parse::<ItemStruct>(input_clone).is_ok() {
        return sql_namespace_impl(args, input);
    }
    generate_mapper_call(args, input)
}

/// Handles `#[sql]` when applied to a struct.
///
/// It injects a `NAMESPACE` constant into the struct's implementation, which
/// is then used by functions within the same struct.
fn sql_namespace_impl(args: TokenStream, input: TokenStream) -> TokenStream {
    let sql_args = parse_macro_input!(args as SqlArgs);
    let namespace = sql_args
        .namespace
        .or(sql_args.value)
        .expect("Namespace is required for struct usage: #[sql(\"my_namespace\")]");

    let item_struct = parse_macro_input!(input as ItemStruct);
    let struct_name = &item_struct.ident;

    let expanded = quote! {
        #item_struct

        impl #struct_name {
            /// The default XML namespace for SQL statements associated with this struct.
            pub const NAMESPACE: &'static str = #namespace;
        }
    };

    TokenStream::from(expanded)
}

/// Handles `#[sql]` when applied to a function.
///
/// It transforms the function body to:
/// 1. Serialize function arguments into a temporary structure.
/// 2. Define a local `exec!()` macro that calls the appropriate `uorm` mapper.
/// 3. Execute the original function block (which is expected to call `exec!()`).
fn generate_mapper_call(args: TokenStream, input: TokenStream) -> TokenStream {
    let sql_args = parse_macro_input!(args as SqlArgs);
    let item_fn = parse_macro_input!(input as ItemFn);

    let fn_name = &item_fn.sig.ident;
    let fn_args = &item_fn.sig.inputs;
    let output = &item_fn.sig.output;
    let vis = &item_fn.vis;
    let block = &item_fn.block;

    // Force the generated function to be `async fn` so `exec!()` can use `.await` directly.
    let async_token = quote! { async };

    // Determine the SQL ID: priority given to explicit `id`, then positional `value`, then function name.
    let id = sql_args
        .id
        .or(sql_args.value)
        .unwrap_or_else(|| fn_name.to_string());
    
    // Determine the database name, defaulting to "default".
    let db_name = sql_args.database.unwrap_or_else(|| "default".to_string());

    // Prepare fields for the anonymous arguments struct that will be serialized.
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

    let (args_struct_def, args_struct_init) = if struct_fields.is_empty() {
        (
            quote! {
                #[derive(serde::Serialize)]
                struct __UormMapperArgs;
            },
            quote! {
                let __uorm_args = __UormMapperArgs;
            },
        )
    } else {
        (
            quote! {
                #[derive(serde::Serialize)]
                struct __UormMapperArgs<'a> {
                    #(#struct_fields),*
                }
            },
            quote! {
                let __uorm_args = __UormMapperArgs {
                    #(#field_inits),*
                };
            },
        )
    };

    // The method to call on the mapper (usually 'execute' or 'query').
    // Note: The macro currently hardcodes 'execute', but the actual behavior
    // is often determined by the return type in more complex implementations.
    let method_ident = syn::Ident::new("execute", Span::call_site());
    let id_lit = LitStr::new(&id, Span::call_site());
    let db_name_lit = LitStr::new(&db_name, Span::call_site());

    // Determine the namespace: either explicitly provided or retrieved from the struct's `NAMESPACE` constant.
    let namespace_tokens = if let Some(ns) = sql_args.namespace {
        let ns_lit = LitStr::new(&ns, Span::call_site());
        quote! { #ns_lit }
    } else {
        quote! { Self::NAMESPACE }
    };

    let expanded = quote! {
        #vis #async_token fn #fn_name(#fn_args) #output {
            /// Temporary structure used to serialize function arguments for the SQL template.
            #args_struct_def
            #args_struct_init
            let __uorm_namespace: &'static str = #namespace_tokens;
            let __uorm_id: &'static str = #id_lit;
            let __uorm_db_name: &'static str = #db_name_lit;

            // Inject a local `exec!()` macro into the function body.
            // This local macro captures the context (namespace, id, db_name) and 
            // performs the actual database call.
            macro_rules! exec {
                () => {{
                    let __uorm_sql_id = format!("{}.{}", __uorm_namespace, __uorm_id);
                    let __uorm_mapper = uorm::driver_manager::UORM
                        .mapper_by_name(__uorm_db_name)
                        .expect("Database driver not found");
                    __uorm_mapper.#method_ident(&__uorm_sql_id, &__uorm_args).await
                }};
            }

            #block
        }
    };

    TokenStream::from(expanded)
}
