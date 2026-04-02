//! Actor-scoped content, context, and policy storage backed by fjall.

mod content;
mod fjall_ext;
mod store;

pub use self::content::ContentHandle;
pub use self::store::Registry;
