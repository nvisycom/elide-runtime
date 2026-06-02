#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod content;
pub mod context;
pub mod nlp;
mod recognizer;
pub use self::recognizer::{Context, ImageData, ModalityData, Recognizer, TextData};

#[cfg(feature = "http")]
#[cfg_attr(docsrs, doc(cfg(feature = "http")))]
pub mod http;

mod error;
pub use self::error::{Error, ErrorKind, Result};
