#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(feature = "internal_audio")]
pub mod audio;
mod decode;
#[cfg(feature = "internal_image")]
pub mod image;
#[cfg(feature = "internal_rich")]
pub mod rich;
#[cfg(feature = "internal_tabular")]
pub mod tabular;
#[cfg(feature = "internal_text")]
pub mod text;

pub use self::decode::decode;
