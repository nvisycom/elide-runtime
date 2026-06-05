#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod content;
pub mod context;
pub mod entity;
pub mod extraction;
pub mod modality;
pub mod primitive;
pub mod recognition;
pub mod redaction;

pub use self::extraction::{
    Artifacts, DataAt, Extractor, ExtractorOutput, ModalityExtraction, RedactAt, Redactions, Span,
    TextAt,
};
pub use self::modality::{AudioData, ImageData, ModalityData, TextData};
pub use self::recognition::{EntityRecognizer, Hint, LabelMap, RecognizerInput, RecognizerOutput};

#[cfg(feature = "http")]
#[cfg_attr(docsrs, doc(cfg(feature = "http")))]
pub mod http;

mod error;
pub use self::error::{Error, ErrorKind, Result};
