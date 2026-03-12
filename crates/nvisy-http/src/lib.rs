#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod client;
mod middleware;

pub use self::client::{HttpClient, HttpConfig};

#[doc(hidden)]
pub mod prelude;
