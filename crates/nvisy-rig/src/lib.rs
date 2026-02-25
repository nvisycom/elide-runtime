#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod backend;
pub mod bridge;
pub mod agent;

pub mod prelude;

// Flat re-exports for ergonomics.
pub use backend::{LlmBackend, LlmConfig};
pub use bridge::{EntityParser, RigBackend, RigBackendConfig};
