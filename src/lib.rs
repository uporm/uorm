pub mod driver_manager;
pub mod error;
pub mod executor;
#[doc(hidden)]
pub mod mapper_loader;
mod page;
pub(crate) mod tpl;
pub mod udbc;


#[doc(hidden)]
pub use ctor;
pub use uorm_macros::mapper_assets;
pub use uorm_macros::sql;
pub use uorm_macros::transaction;
use crate::error::DbError;

pub type Result<T> = std::result::Result<T, DbError>;

#[macro_export]
macro_rules! exec {
    () => {
        unimplemented!("This macro should be handled by the sql attribute macros")
    };
}
