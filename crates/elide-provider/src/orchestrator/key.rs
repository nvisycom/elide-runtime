//! [`KeyConfig`]: which cryptographic key provider to build, as
//! data.
//!
//! Every other part of a request's configuration is already
//! serializable, because it *describes* something the engine
//! compiles later. A [`KeyProvider`] is not: it is behaviour, a
//! trait object answering "the key for entities carrying this
//! label". So this names a provider and builds one, rather than
//! deserializing it.
//!
//! Supplied per request, not per deployment. A key belongs to the
//! caller asking for redaction, not to the process serving them: a
//! host holding one tenant's key on the provider would have to
//! rebuild the provider per tenant, and could not run two tenants
//! through the same one. So a [`KeyConfig`] travels with the
//! request that needs it, and a request whose policies name no
//! keyed operator carries none.
//!
//! Elide ships one [`KeyProvider`], so this has one variant. It is
//! `#[non_exhaustive]`: a deployment needing per-label keys, a
//! keyset with rotation, or a network-backed store implements
//! [`KeyProvider`] itself and passes the instance instead.
//!
//! [`KeyProvider`]: elide::redaction::operators::KeyProvider

use std::sync::Arc;

use elide::redaction::operators::{KeyProvider, StaticKey};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which key provider the `HmacHash` and `Encrypt` operators
/// resolve through for one request.
///
/// One provider backs both. A policy naming either operator
/// without one supplied fails at request-compile time, naming the
/// policy and the operator, rather than redacting with some
/// default key.
///
/// # Secrets
///
/// [`Static`] carries the key itself, so a serialized `KeyConfig`
/// *is* key material and must be handled as such: it belongs in a
/// request body over an encrypted channel, never in a log, a trace,
/// or anything persisted in the clear. [`Debug`] prints no key
/// bytes, but that is a courtesy, not a boundary.
///
/// [`Static`]: Self::Static
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[non_exhaustive]
pub enum KeyConfig {
    /// One key for every label, for the length of this request.
    ///
    /// The common case, and elide's own [`StaticKey`]. Per-label
    /// keys need a [`KeyProvider`] of the caller's own.
    #[serde(rename_all = "camelCase")]
    Static {
        /// The key. AES-256 requires exactly 32 bytes; HMAC accepts
        /// any length.
        key: Vec<u8>,
    },
}

impl KeyConfig {
    /// Build the provider this config names.
    #[must_use]
    pub fn build(&self) -> Arc<dyn KeyProvider> {
        match self {
            Self::Static { key } => Arc::new(StaticKey::new(key.clone())),
        }
    }
}

/// Never prints key material.
impl std::fmt::Debug for KeyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Static { .. } => f.debug_struct("Static").finish_non_exhaustive(),
        }
    }
}
