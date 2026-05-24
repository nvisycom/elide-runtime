#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(any(feature = "wav", feature = "mp3"))]
pub mod audio;
mod decode;
#[cfg(any(feature = "png", feature = "jpeg", feature = "tiff"))]
pub mod image;
#[cfg(any(feature = "pdf", feature = "docx"))]
pub mod rich;
#[cfg(any(feature = "csv", feature = "xlsx"))]
pub mod tabular;
#[cfg(any(feature = "txt", feature = "json", feature = "markdown", feature = "html"))]
pub mod text;

pub use self::decode::decode;
