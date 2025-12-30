pub mod driver_manager;
pub mod error;
pub mod executor;
#[doc(hidden)]
pub mod mapper_loader;
mod page;
pub(crate) mod tpl;
pub mod udbc;

use crate::error::DbError;
#[doc(hidden)]
pub use ctor;
pub use udbc::value::{FromValue, ToValue, Value};
pub use uorm_macros::Param;
pub use uorm_macros::mapper_assets;
pub use uorm_macros::param;
pub use uorm_macros::sql;
pub use uorm_macros::transaction;

pub type Result<T> = std::result::Result<T, DbError>;

#[macro_export]
macro_rules! exec {
    () => {
        unimplemented!("This macro should be handled by the sql attribute macros")
    };
}
