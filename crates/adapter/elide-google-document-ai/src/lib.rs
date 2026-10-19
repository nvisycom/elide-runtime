#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;
mod ocr;

/// The [`google_cloud_documentai_v1`] SDK this backend is built on.
///
/// Re-exported because [`DocumentAiOcr::new`] takes its client type: the
/// SDK is already part of this crate's public surface, so a consumer
/// configuring one should not have to name the dependency twice.
pub use google_cloud_documentai_v1 as document_ai;

pub use self::ocr::DocumentAiOcr;
