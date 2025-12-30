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
            return Ok(SqlArgs {
                value,
                id,
                database,
                namespace,
            });
        }

        // Try to parse an optional positional string literal first.
        if input.peek(LitStr) {
            let s: LitStr = input.parse()?;
            value = Some(s.value());

            if input.is_empty() {
                return Ok(SqlArgs {
                    value,
                    id,
                    database,
                    namespace,
                });
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

        Ok(SqlArgs {
            value,
            id,
            database,
            namespace,
        })
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

fn is_primitive_or_wrapper(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(type_path) => {
            if let Some(ident) = type_path.path.get_ident() {
                let s = ident.to_string();
                matches!(
                    s.as_str(),
                    "bool"
                        | "char"
                        | "i8"
                        | "i16"
                        | "i32"
                        | "i64"
                        | "i128"
                        | "isize"
                        | "u8"
                        | "u16"
                        | "u32"
                        | "u64"
                        | "u128"
                        | "usize"
                        | "f32"
                        | "f64"
                        | "str"
                        | "String"
                        | "Vec"
                        | "Option"
                )
            } else if let Some(last) = type_path.path.segments.last() {
                let s = last.ident.to_string();
                matches!(s.as_str(), "String" | "Vec" | "Option")
            } else {
                false
            }
        }
        syn::Type::Reference(type_ref) => is_primitive_or_wrapper(&type_ref.elem),
        _ => true,
    }
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
    let raw_id = sql_args
        .id
        .or(sql_args.value)
        .unwrap_or_else(|| fn_name.to_string());

    // Check if raw_id contains a dot to infer namespace
    let (inferred_namespace, final_id) = if let Some(idx) = raw_id.find('.') {
        (Some(raw_id[..idx].to_string()), raw_id[idx+1..].to_string())
    } else {
        (None, raw_id)
    };

    // Determine the database name, defaulting to "default".
    let db_name = sql_args.database.unwrap_or_else(|| "default".to_string());

    // Prepare fields for the anonymous arguments struct that will be serialized.
    let mut struct_fields = Vec::new();
    let mut field_inits = Vec::new();

    let mut use_arg_directly = false;
    let mut direct_arg_ident = None;
    
    // Check for parameter mapping
    let mut param_mappings = std::collections::HashMap::new();
    let mut use_param_mapping = false;

    for attr in &item_fn.attrs {
        if attr.path().is_ident("uorm_internal_param_mapping") {
            use_param_mapping = true;
            if let Meta::List(meta_list) = &attr.meta {
                 let nested = meta_list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated);
                 if let Ok(metas) = nested {
                     for meta in metas {
                         if let Meta::NameValue(nv) = meta {
                             if let Some(ident) = nv.path.get_ident() {
                                 if let Expr::Lit(expr_lit) = &nv.value {
                                     if let Lit::Str(lit_str) = &expr_lit.lit {
                                         param_mappings.insert(ident.to_string(), lit_str.value());
                                     }
                                 }
                             }
                         }
                     }
                 }
            }
        }
    }

    // Check if we should unwrap a single struct argument
    let typed_args: Vec<&syn::PatType> = fn_args
        .iter()
        .filter_map(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                Some(pat_type)
            } else {
                None
            }
        })
        .collect();

    if !use_param_mapping && typed_args.len() == 1 {
        let arg = typed_args[0];
        if !is_primitive_or_wrapper(&arg.ty) {
            if let syn::Pat::Ident(pat_ident) = &*arg.pat {
                use_arg_directly = true;
                direct_arg_ident = Some(&pat_ident.ident);
            }
        }
    }

    if !use_arg_directly && !use_param_mapping {
        for arg in fn_args {
            if let syn::FnArg::Typed(pat_type) = arg
                && let syn::Pat::Ident(pat_ident) = &*pat_type.pat
            {
                let ident = &pat_ident.ident;
                let ty = &pat_type.ty;

                // Check if ty is a reference to handle `&T` arguments correctly.
                // If it is a reference, we use the inner type for the struct field
                // and initialize it directly with the argument (which is already a reference).
                if let syn::Type::Reference(type_ref) = &**ty {
                    let inner_ty = &type_ref.elem;
                    struct_fields.push(quote! { #ident: &'a #inner_ty });
                    field_inits.push(quote! { #ident: #ident });
                } else {
                    struct_fields.push(quote! { #ident: &'a #ty });
                    field_inits.push(quote! { #ident: &#ident });
                }
            }
        }
    }

    let (args_struct_def, args_struct_init) = if use_param_mapping {
        let mut inserts = Vec::new();
        for arg in fn_args {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    let ident = &pat_ident.ident;
                    let ident_str = ident.to_string();
                    let key = param_mappings.get(&ident_str).cloned().unwrap_or(ident_str);
                    inserts.push(quote! {
                        __uorm_map.insert(#key.to_string(), uorm::udbc::value::ToValue::to_value(&#ident));
                    });
                }
            }
        }
        
        (
            quote! {},
            quote! {
                let mut __uorm_map = std::collections::HashMap::new();
                #(#inserts)*
                let __uorm_args = uorm::udbc::value::Value::Map(__uorm_map);
            },
        )
    } else if use_arg_directly {
        let ident = direct_arg_ident.unwrap();
        (
            quote! {},
            quote! {
                let __uorm_args = &#ident;
            },
        )
    } else if struct_fields.is_empty() {
        (
            quote! {
                struct __UormMapperArgs;
                impl uorm::udbc::value::ToValue for __UormMapperArgs {
                    fn to_value(&self) -> uorm::udbc::value::Value {
                        uorm::udbc::value::Value::Map(std::collections::HashMap::new())
                    }
                }
            },
            quote! {
                let __uorm_args = __UormMapperArgs;
            },
        )
    } else {
        let mut to_value_inserts = Vec::new();
        for arg in fn_args {
            if let syn::FnArg::Typed(pat_type) = arg
                && let syn::Pat::Ident(pat_ident) = &*pat_type.pat
            {
                let ident = &pat_ident.ident;
                let ident_str = ident.to_string();
                to_value_inserts.push(quote! {
                     map.insert(#ident_str.to_string(), uorm::udbc::value::ToValue::to_value(&self.#ident));
                });
            }
        }

        (
            quote! {
                struct __UormMapperArgs<'a> {
                    #(#struct_fields),*
                }
                impl<'a> uorm::udbc::value::ToValue for __UormMapperArgs<'a> {
                    fn to_value(&self) -> uorm::udbc::value::Value {
                        let mut map = std::collections::HashMap::new();
                        #(#to_value_inserts)*
                        uorm::udbc::value::Value::Map(map)
                    }
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
    let id_lit = LitStr::new(&final_id, Span::call_site());
    let db_name_lit = LitStr::new(&db_name, Span::call_site());

    // Determine the namespace: either explicitly provided or retrieved from the struct's `NAMESPACE` constant.
    let namespace_tokens = if let Some(ns) = sql_args.namespace {
        let ns_lit = LitStr::new(&ns, Span::call_site());
        quote! { #ns_lit }
    } else if let Some(ns) = inferred_namespace {
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
                    let __uorm_mapper = uorm::driver_manager::U
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
