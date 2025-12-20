mod assets;
mod sql;

use proc_macro::TokenStream;

#[proc_macro]
pub fn mapper_assets(input: TokenStream) -> TokenStream {
    assets::mapper_assets_impl(input)
}

#[proc_macro_attribute]
pub fn sql_namespace(args: TokenStream, input: TokenStream) -> TokenStream {
    sql::sql_namespace_impl(args, input)
}

#[proc_macro_attribute]
pub fn sql_list(args: TokenStream, input: TokenStream) -> TokenStream {
    sql::sql_list_impl(args, input)
}

#[proc_macro_attribute]
pub fn sql_get(args: TokenStream, input: TokenStream) -> TokenStream {
    sql::sql_get_impl(args, input)
}

#[proc_macro_attribute]
pub fn sql_insert(args: TokenStream, input: TokenStream) -> TokenStream {
    sql::sql_insert_impl(args, input)
}

#[proc_macro_attribute]
pub fn sql_update(args: TokenStream, input: TokenStream) -> TokenStream {
    sql::sql_update_impl(args, input)
}

#[proc_macro_attribute]
pub fn sql_delete(args: TokenStream, input: TokenStream) -> TokenStream {
    sql::sql_delete_impl(args, input)
}
