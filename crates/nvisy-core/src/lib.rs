#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod content;
pub mod context;
pub mod nlp;
pub mod recognition;
pub use self::recognition::{Context, EntityRecognizer, ImageData, ModalityData, TextData};

#[cfg(feature = "http")]
#[cfg_attr(docsrs, doc(cfg(feature = "http")))]
pub mod http;

mod error;
pub use self::error::{Error, ErrorKind, Result};
