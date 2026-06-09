//! Actor-scoped content, context, and policy storage backed by fjall.
//!
//! The [`Registry`] stores content, contexts, policies, and audit
//! trails in a fjall database. All entries are actor-scoped via
//! a composite key.
//!
//! [`ResourceCache`] provides generic ref-counted caching on top of
//! the store, used for contexts and policies.

mod composite_key;
mod content_handle;
mod fjall_ext;
mod paged;
mod registry_store;
mod resource_cache;

pub use self::content_handle::ContentHandle;
pub use self::paged::PagedResult;
pub use self::registry_store::Registry;
pub use self::resource_cache::{ResourceCache, ResourceGuard};
