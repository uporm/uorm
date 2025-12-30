mod assets;
mod sql;
mod transaction;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, DeriveInput, ItemFn, LitStr};
#[proc_macro]
pub fn mapper_assets(input: TokenStream) -> TokenStream {
    assets::mapper_assets_impl(input)
}

#[proc_macro_attribute]
pub fn sql(args: TokenStream, input: TokenStream) -> TokenStream {
    sql::sql_impl(args, input)
}

#[proc_macro_attribute]
pub fn transaction(args: TokenStream, input: TokenStream) -> TokenStream {
    transaction::transaction_impl(args, input)
}
// --- 派生宏部分 ---
#[proc_macro_derive(Param, attributes(param))]
pub fn derive_param(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    // 确保只能用于 Struct
    let fields = match input.data {
        syn::Data::Struct(data) => data.fields,
        _ => return syn::Error::new_spanned(name, "Param only supports structs").to_compile_error().into(),
    };

    // 1. 生成 ToValue 逻辑
    let to_inserts = fields.iter().map(|f| {
        let field_name = f.ident.as_ref().unwrap();
        let (key, ignore) = parse_field_attrs(f);
        if ignore {
            quote! {}
        } else {
            quote! { map.insert(#key.to_string(), uorm::udbc::value::ToValue::to_value(&self.#field_name)); }
        }
    });

    // 2. 生成 FromValue 逻辑
    let from_fields = fields.iter().map(|f| {
        let field_name = f.ident.as_ref().unwrap();
        let (key, ignore) = parse_field_attrs(f);

        if ignore {
            quote! { #field_name: Default::default(), }
        } else {
            quote! {
                #field_name: {
                    let v = map.remove(#key).unwrap_or(uorm::udbc::value::Value::Null);
                    uorm::udbc::value::FromValue::from_value(v)?
                },
            }
        }
    });

    TokenStream::from(quote! {
        impl uorm::udbc::value::ToValue for #name {
            fn to_value(&self) -> uorm::udbc::value::Value {
                let mut map = std::collections::HashMap::new();
                #(#to_inserts)*
                uorm::udbc::value::Value::Map(map)
            }
        }
        impl uorm::udbc::value::FromValue for #name {
            fn from_value(v: uorm::udbc::value::Value) -> std::result::Result<Self, uorm::error::DbError> {
                if let uorm::udbc::value::Value::Map(mut map) = v {
                    Ok(Self { #(#from_fields)* })
                } else {
                    Err(uorm::error::DbError::TypeMismatch(format!("Expected Map, got {:?}", v)))
                }
            }
        }
    })
}

// 辅助函数：解析结构体字段属性 (syn 2.0 风格)
fn parse_field_attrs(field: &syn::Field) -> (String, bool) {
    let mut name = field.ident.as_ref().unwrap().to_string();
    let mut ignore = false;

    for attr in &field.attrs {
        if attr.path().is_ident("param") {
            // 1. 尝试解析为单字符串: #[param("custom_name")]
            if let Ok(s) = attr.parse_args::<LitStr>() {
                name = s.value();
                continue;
            }

            // 2. 解析 k-v 形式或 flag: #[param(ignore)], #[param(rename="xxx")]
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("ignore") {
                    ignore = true;
                } else if meta.path.is_ident("rename") { // 显式 rename="xxx"
                    let value = meta.value()?;
                    let s: LitStr = value.parse()?;
                    name = s.value();
                }
                Ok(())
            });
        }
    }
    (name, ignore)
}

// --- 属性宏部分 ---
/// 属性宏：#[param(user_id="id")]
#[proc_macro_attribute]
pub fn param(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut func = parse_macro_input!(input as ItemFn);
    let args_ts = proc_macro2::TokenStream::from(args);

    let attr: syn::Attribute = syn::parse_quote! {
        #[uorm_internal_param_mapping(#args_ts)]
    };
    func.attrs.push(attr);

    TokenStream::from(func.into_token_stream())
}
