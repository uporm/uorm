pub mod driver_manager;
pub mod error;
pub mod executor;
pub mod mapper_loader;
pub(crate) mod tpl;
pub mod transaction;
pub mod udbc;

#[doc(hidden)]
pub use ctor;
pub use uorm_macros::mapper_assets;
pub use uorm_macros::sql;

#[macro_export]
macro_rules! exec {
    () => {
        unimplemented!("This macro should be handled by the sql attribute macros")
    };
}
