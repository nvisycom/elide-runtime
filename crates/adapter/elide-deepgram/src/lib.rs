#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;
mod stt;

/// The [`deepgram`] SDK this backend is built on.
///
/// Re-exported because [`DeepgramStt::from_client`] takes a
/// `deepgram::Deepgram` and [`DeepgramStt::with_model`] a
/// `deepgram::common::options::Model`: the SDK is already part of this
/// crate's public surface, so a consumer should not have to name the
/// dependency twice.
pub use deepgram;

pub use self::stt::DeepgramStt;
