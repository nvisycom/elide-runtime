//! Actor-scoped content and context storage backed by fjall.
//!
//! This crate provides [`Registry`], a unified store that manages both
//! content files and detection contexts. Every resource is scoped by an
//! [`ActorId`], so listing and reading are inherently actor-isolated at
//! the database level via composite keys.
//!
//! # Core Types
//!
//! - [`Registry`]: Shared, clonable handle to the fjall database
//! - [`ActorId`]: UUID-based actor identity newtype
//! - [`ContentHandle`]: Lightweight async handle to stored content
//! - [`ContextHandle`]: Lightweight async handle to a stored context

mod actor;
mod content_handle;
mod context_handle;
mod registry;

pub use actor::ActorId;
pub use content_handle::ContentHandle;
pub use context_handle::ContextHandle;
pub use registry::Registry;
