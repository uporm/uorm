mod assets;
use proc_macro::TokenStream;

#[proc_macro]
pub fn mapper_assets(input: TokenStream) -> TokenStream {
    assets::mapper_assets_impl(input)
}
