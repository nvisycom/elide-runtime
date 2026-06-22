#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod client;
pub mod error;

pub use self::client::{BentoClient, BentoParams};
pub use self::error::BentoError;
