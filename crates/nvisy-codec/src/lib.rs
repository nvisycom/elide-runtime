#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(not(any(
    feature = "text",
    feature = "tabular",
    feature = "image",
    feature = "audio",
)))]
compile_error!(
    "nvisy-codec requires at least one modality feature: \
     `text`, `tabular`, `image`, or `audio`"
);

pub mod core;
pub mod document;
pub mod handler;

pub use self::core::{CodecRegistry, ErasedLoader, Format, FormatId, LoaderAdapter, WrapUntyped};
pub use self::document::{DocumentHandle, UntypedDocumentHandle};
