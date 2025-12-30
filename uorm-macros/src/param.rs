use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{ToTokens, quote};
use syn::{DeriveInput, ItemFn, LitStr, parse_macro_input};

pub fn derive_param_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let fields = match input.data {
        syn::Data::Struct(data) => match data.fields {
            syn::Fields::Named(fields) => fields.named,
            _ => {
                return syn::Error::new_spanned(
                    name,
                    "Param only supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(name, "Param only supports structs")
                .to_compile_error()
                .into();
        }
    };

    let case_helpers = quote! {
        fn snake_to_camel(s: &str) -> String {
            let mut out = String::new();
            let mut parts = s.split('_').filter(|p| !p.is_empty());
            if let Some(first) = parts.next() {
                out.push_str(first);
            }
            for part in parts {
                let mut chars = part.chars();
                if let Some(c0) = chars.next() {
                    out.extend(c0.to_uppercase());
                    out.push_str(chars.as_str());
                }
            }
            out
        }

        fn camel_to_snake(s: &str) -> String {
            let mut out = String::new();
            let mut iter = s.chars().peekable();
            let mut prev_is_lower_or_digit = false;

            while let Some(ch) = iter.next() {
                if ch == '_' {
                    if !out.ends_with('_') {
                        out.push('_');
                    }
                    prev_is_lower_or_digit = false;
                    continue;
                }

                if ch.is_uppercase() {
                    let next = iter.peek().copied();
                    let next_is_lower_or_digit = next
                        .map(|n| n.is_lowercase() || n.is_numeric())
                        .unwrap_or(false);

                    if !out.is_empty() && (prev_is_lower_or_digit || next_is_lower_or_digit) && !out.ends_with('_') {
                        out.push('_');
                    }

                    out.extend(ch.to_lowercase());
                    prev_is_lower_or_digit = false;
                } else {
                    out.push(ch);
                    prev_is_lower_or_digit = ch.is_lowercase() || ch.is_numeric();
                }
            }

            out
        }
    };

    let to_inserts = fields.iter().map(|f| {
        let field_name = f.ident.as_ref().unwrap();
        let (key, ignore) = parse_field_attrs(f);
        let key_lit = LitStr::new(&key, Span::call_site());
        if ignore {
            quote! {}
        } else {
            quote! {
                {
                    let key: &str = #key_lit;
                    let value = uorm::udbc::value::ToValue::to_value(&self.#field_name);
                    map.insert(key.to_string(), value.clone());

                    let camel = snake_to_camel(key);
                    if camel != key {
                        map.entry(camel)
                            .or_insert_with(|| value.clone());
                    }

                    let snake = camel_to_snake(key);
                    if snake != key {
                        map.entry(snake)
                            .or_insert_with(|| value.clone());
                    }
                }
            }
        }
    });

    let from_fields = fields.iter().map(|f| {
        let field_name = f.ident.as_ref().unwrap();
        let (key, ignore) = parse_field_attrs(f);
        let key_lit = LitStr::new(&key, Span::call_site());

        if ignore {
            quote! { #field_name: Default::default(), }
        } else {
            quote! {
                #field_name: {
                    let key: &str = #key_lit;
                    let mut v = map.remove(key);

                    if v.is_none() {
                        let camel = snake_to_camel(key);
                        if camel != key {
                            v = map.remove(camel.as_str());
                        }
                    }

                    if v.is_none() {
                        let snake = camel_to_snake(key);
                        if snake != key {
                            v = map.remove(snake.as_str());
                        }
                    }

                    let v = v.unwrap_or(uorm::udbc::value::Value::Null);
                    uorm::udbc::value::FromValue::from_value(v)?
                },
            }
        }
    });

    TokenStream::from(quote! {
        impl uorm::udbc::value::ToValue for #name {
            fn to_value(&self) -> uorm::udbc::value::Value {
                #case_helpers

                let mut map = std::collections::HashMap::new();
                #(#to_inserts)*
                uorm::udbc::value::Value::Map(map)
            }
        }
        impl uorm::udbc::value::FromValue for #name {
            fn from_value(v: uorm::udbc::value::Value) -> std::result::Result<Self, uorm::error::DbError> {
                if let uorm::udbc::value::Value::Map(mut map) = v {
                    #case_helpers

                    Ok(Self { #(#from_fields)* })
                } else {
                    Err(uorm::error::DbError::TypeMismatch(format!("Expected Map, got {:?}", v)))
                }
            }
        }
    })
}

fn parse_field_attrs(field: &syn::Field) -> (String, bool) {
    let mut name = field.ident.as_ref().unwrap().to_string();
    let mut ignore = false;

    for attr in &field.attrs {
        if attr.path().is_ident("param") {
            if let Ok(s) = attr.parse_args::<LitStr>() {
                name = s.value();
                continue;
            }

            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("ignore") {
                    ignore = true;
                } else if meta.path.is_ident("rename") {
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

pub fn param_impl(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut func = parse_macro_input!(input as ItemFn);
    let args_ts = proc_macro2::TokenStream::from(args);

    let attr: syn::Attribute = syn::parse_quote! {
        #[uorm_internal_param_mapping(#args_ts)]
    };
    func.attrs.push(attr);

    TokenStream::from(func.into_token_stream())
}
