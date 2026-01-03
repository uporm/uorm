use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Expr, ItemFn, Lit, LitStr, Meta, Result, Token, parse::Parse, parse::ParseStream,
    parse_macro_input, punctuated::Punctuated,
};

struct TransactionArgs {
    database: Option<String>,
}

impl Parse for TransactionArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut database = None;
        if !input.is_empty() {
            let metas: Punctuated<Meta, Token![,]> = Punctuated::parse_terminated(input)?;
            for meta in metas {
                if let Meta::NameValue(nv) = meta
                    && let Some(ident) = nv.path.get_ident()
                    && ident == "database"
                    && let Expr::Lit(expr_lit) = &nv.value
                    && let Lit::Str(lit_str) = &expr_lit.lit
                {
                    database = Some(lit_str.value());
                }
            }
        }
        Ok(TransactionArgs { database })
    }
}

pub fn transaction_impl(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as TransactionArgs);
    let mut func = parse_macro_input!(input as ItemFn);

    let block = &func.block;

    let db_name = args.database.unwrap_or_else(|| "default".to_string());
    let db_name_lit = LitStr::new(&db_name, proc_macro2::Span::call_site());
    let new_block = quote! {
        {
            let __uorm_mapper = uorm::driver_manager::U
                .mapper_by_name(#db_name_lit)
                .expect("Database driver not found");
            let __uorm_session = uorm::executor::session::Session::new(__uorm_mapper.pool.clone());

            let __uorm_tx_started = !__uorm_session.is_transaction_active();
            if __uorm_tx_started {
                if let Err(e) = __uorm_session.begin().await {
                    return uorm::TransactionResult::from_db_error(e);
                }
            }

            let result = (async #block).await;

            if __uorm_tx_started {
                if uorm::TransactionResult::is_ok(&result) {
                    if let Err(e) = __uorm_session.commit().await {
                        return uorm::TransactionResult::from_db_error(e);
                    }
                } else {
                    let _ = __uorm_session.rollback().await;
                }
            }

            result
        }
    };

    func.block = syn::parse2(new_block).expect("Failed to parse new block");

    quote! {
        #func
    }
    .into()
}
