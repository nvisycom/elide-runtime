#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod content;
pub mod context;
pub mod entity;
pub mod modality;
pub mod nlp;
pub mod primitive;
pub mod recognition;
mod value_at;
pub use self::recognition::{Context, EntityRecognizer, ImageData, ModalityData, TextData};
pub use self::value_at::ValueAt;

#[cfg(feature = "http")]
#[cfg_attr(docsrs, doc(cfg(feature = "http")))]
pub mod http;

mod error;
pub use self::error::{Error, ErrorKind, Result};
