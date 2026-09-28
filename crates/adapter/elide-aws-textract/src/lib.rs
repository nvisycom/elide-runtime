#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;
mod ocr;

/// The [`aws_sdk_textract`] SDK this backend is built on.
///
/// Re-exported because [`TextractOcr::new`] takes its client type: the
/// SDK is already part of this crate's public surface, so a consumer
/// configuring one should not have to name the dependency twice.
pub use aws_sdk_textract as textract;

pub use self::ocr::TextractOcr;
