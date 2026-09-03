#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;
mod stt;

/// The [`gladia`] SDK this backend is built on.
///
/// Re-exported because [`GladiaStt::from_client`] takes a `gladia::Client`:
/// the SDK is already part of this crate's public surface, so a consumer
/// configuring one should not have to name the dependency twice.
pub use gladia;

pub use self::stt::GladiaStt;
