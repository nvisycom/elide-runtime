//! Content encryption and decryption.
//!
//! Provides [`CryptoService`] for AES-256-GCM encryption/decryption,
//! [`KeyProvider`] for key resolution, and a self-describing wire format.

mod provider;
mod service;
mod wire;

pub use self::provider::{KeyProvider, StaticKeyProvider};
pub use self::service::CryptoService;
pub use self::wire::{EncryptedContent, EncryptionAlgorithm};
