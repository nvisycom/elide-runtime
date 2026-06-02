#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod content;
pub mod context;
pub mod entity;
pub mod extraction;
pub mod modality;
pub mod nlp;
pub mod primitive;
pub mod recognition;
pub use self::extraction::{Extractor, ValueAt};
pub use self::recognition::{
    AudioData, EntityRecognizer, ImageData, ModalityData, RecognizerInput, TextData,
};

#[cfg(feature = "http")]
#[cfg_attr(docsrs, doc(cfg(feature = "http")))]
pub mod http;

mod error;
pub use self::error::{Error, ErrorKind, Result};
