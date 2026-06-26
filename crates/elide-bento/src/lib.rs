#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;

#[cfg(feature = "ner")]
#[cfg_attr(docsrs, doc(cfg(feature = "ner")))]
pub mod ner;

#[cfg(feature = "ocr")]
#[cfg_attr(docsrs, doc(cfg(feature = "ocr")))]
pub mod ocr;

#[cfg(feature = "ner")]
pub use self::ner::BentoNer;
#[cfg(feature = "ocr")]
pub use self::ocr::BentoOcr;
