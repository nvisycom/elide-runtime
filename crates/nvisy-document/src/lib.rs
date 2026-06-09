#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod core;
pub mod document;
pub mod modality;
pub mod phases;
pub mod pipeline;
pub mod policy;
pub mod provenance;
pub mod validation;

pub use nvisy_codec::content::{
    Content, ContentData, ContentDescriptor, ContentDigest, ContentRecord, ContentSource,
};
