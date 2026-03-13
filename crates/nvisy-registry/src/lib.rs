#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod handler;
mod registry;

#[doc(hidden)]
pub mod prelude;

pub use self::handler::{ContentHandle, ContextHandle};
pub use self::registry::Registry;
