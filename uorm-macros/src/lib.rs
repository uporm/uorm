mod assets;
mod sql;
mod transaction;

use proc_macro::TokenStream;

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
