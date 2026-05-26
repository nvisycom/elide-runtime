#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(not(any(
    feature = "text",
    feature = "tabular",
    feature = "image",
    feature = "audio",
    feature = "rich",
)))]
compile_error!(
    "nvisy-codec requires at least one modality feature: \
     `text`, `tabular`, `image`, `audio`, or `rich`"
);

pub mod document;
pub mod handler;

pub use self::document::{DocumentHandle, Located, LocationStream, Span};
