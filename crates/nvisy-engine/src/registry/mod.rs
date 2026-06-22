//! Multi-tenant registry utilities backed by [`fjall`].
//!
//! Only the primitives — [`CompositeKey`] actor scoping, [`fjall`]
//! extension traits, paged iteration — survive the engine rebuild;
//! the higher-level content / audit stores will be redesigned around
//! whatever request / result types engine ends up exposing.

mod composite_key;
mod fjall_ext;
mod paged;

pub(crate) use self::composite_key::CompositeKey;
pub(crate) use self::fjall_ext::{
    FjallDatabaseExt, FjallKeyspaceExt, blocking, not_found,
};
pub(crate) use self::paged::PagedResult;
