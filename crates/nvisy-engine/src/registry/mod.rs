//! Actor-scoped content, context, and policy storage backed by fjall.
//!
//! The [`Registry`] stores content, contexts, policies, and audit
//! trails in a fjall database. All entries are actor-scoped via
//! a composite key.
//!
//! [`ResourceCache`] provides generic ref-counted caching on top of
//! the store, used for contexts and policies.

mod cache;
mod content;
mod fjall_ext;
mod key;
mod store;

pub use self::cache::{ResourceCache, ResourceGuard};
pub use self::content::ContentHandle;
pub use self::store::Registry;
