//! Content encryption and decryption.
//!
//! Provides [`CryptoService`] for AES-256-GCM encryption/decryption,
//! [`KeyProvider`] for key resolution, and a self-describing wire format.

mod provider;
mod service;
mod wire;

// Re-exports will be added when encryption is wired into import/export.
