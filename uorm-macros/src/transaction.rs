use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, LitStr, Result, parse::Parse, parse::ParseStream, parse_macro_input};

struct TransactionArgs {
    session_name: Option<String>,
}

impl Parse for TransactionArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut session_name = None;
        if !input.is_empty() {
            let s: LitStr = input.parse()?;
            session_name = Some(s.value());
        }
        Ok(TransactionArgs { session_name })
    }
}

pub fn transaction_impl(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as TransactionArgs);
    let mut func = parse_macro_input!(input as ItemFn);

    let session_name = args.session_name.unwrap_or_else(|| "session".to_string());
    let session_ident = syn::Ident::new(&session_name, proc_macro2::Span::call_site());

    let block = &func.block;

    // We assume the return type is Result<T, E> where E: From<DbError>
    // We use fully qualified paths where possible, but here we depend on the method availability on session_ident

    let new_block = quote! {
        {
            // Start transaction
            match #session_ident.begin().await {
                Ok(_) => {},
                Err(e) => return Err(e.into()),
            }

            // Execute original body
            let result = (async #block).await;

            match result {
                Ok(val) => {
                     // Commit if successful
                     match #session_ident.commit().await {
                         Ok(_) => Ok(val),
                         Err(e) => Err(e.into()),
                     }
                }
                Err(e) => {
                     // Rollback if error
                     // We ignore rollback errors as we want to return the original error
                     let _ = #session_ident.rollback().await;
                     Err(e)
                }
            }
        }
    };

    func.block = syn::parse2(new_block).expect("Failed to parse new block");

    quote! {
        #func
    }
    .into()
}
