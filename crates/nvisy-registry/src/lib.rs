//! Actor-scoped content and context storage backed by fjall.
//!
//! This crate provides [`Registry`], a unified store that manages both
//! content files and detection contexts. Every resource is scoped by a
//! `Uuid` actor identity, so listing and reading are inherently
//! actor-isolated at the database level via composite keys.
//!
//! # Core Types
//!
//! - [`Registry`]: Shared, clonable handle to the fjall database
//! - [`ContentHandle`]: Lightweight async handle to stored content
//! - [`ContextHandle`]: Lightweight async handle to a stored context

mod store;

#[doc(hidden)]
pub mod prelude;

pub use store::{ContentHandle, ContextHandle, Registry};
