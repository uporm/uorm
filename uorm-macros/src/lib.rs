mod assets;
mod param;
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
// --- 派生宏部分 ---
#[proc_macro_derive(Param, attributes(param))]
pub fn derive_param(input: TokenStream) -> TokenStream {
    param::derive_param_impl(input)
}

// --- 属性宏部分 ---
/// 属性宏：#[param(user_id="id")]
#[proc_macro_attribute]
pub fn param(args: TokenStream, input: TokenStream) -> TokenStream {
    param::param_impl(args, input)
}
