//! Actor-scoped content and audit storage backed by fjall.
//!
//! The [`Registry`] stores content blobs, content metadata,
//! annotations, audits, and detection / redaction results. All
//! entries are actor-scoped via a composite key. Policies are NOT
//! persisted — they are submitted inline on every detection input
//! and snapshotted onto the in-memory detection record.

mod composite_key;
mod content_handle;
mod fjall_ext;
mod paged;
mod registry_store;

pub use self::content_handle::ContentHandle;
pub use self::paged::PagedResult;
pub use self::registry_store::Registry;
