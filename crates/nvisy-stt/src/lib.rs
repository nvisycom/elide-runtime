#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod backend;
pub mod extraction;

pub use self::backend::{
    NoopBackend, SttBackend, SttRequest, SttResponse, TranscribedSegment, TranscribedWord,
};
pub use self::extraction::{SttExtractor, Transcription};
