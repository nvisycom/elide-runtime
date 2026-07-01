#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod extract;
pub mod handler;
pub mod middleware;
pub mod service;

pub use self::handler::error::{Error, ErrorKind, Result};
pub use self::handler::routes;
pub use self::service::{ServiceRuntime, ServiceState};
