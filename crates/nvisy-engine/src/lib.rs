#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod core;
pub mod detection;
pub mod document;
pub mod modality;
pub mod redaction;
pub mod registry;

pub use nvisy_codec::content::{
    Content, ContentData, ContentDescriptor, ContentDigest, ContentRecord, ContentSource,
};
