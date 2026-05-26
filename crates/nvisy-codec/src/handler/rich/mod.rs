//! Rich-document handler glue: [`RichHandle`] is a marker trait that
//! a single trait object can satisfy when its concrete type
//! implements both [`Handle<Text>`] and [`Handle<Image>`] (PDF,
//! DOCX, …). Concrete per-format implementations live in
//! `nvisy-formats`.
//!
//! [`Handle<Text>`]: crate::handler::Handle
//! [`Handle<Image>`]: crate::handler::Handle

mod boxed;

pub use self::boxed::RichHandle;
