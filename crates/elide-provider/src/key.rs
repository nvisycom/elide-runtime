//! [`KeyConfig`]: which cryptographic key provider a deployment
//! wires, as data.
//!
//! Every other part of an engine's configuration is already
//! serializable, because it *describes* something the engine
//! compiles later. A [`KeyProvider`] is not: it is behaviour, a
//! trait object answering "the key for entities carrying this
//! label". So the config names a provider and this module builds
//! one, rather than deserializing it.
//!
//! # Where the secrets are
//!
//! Not in the config. A [`KeyConfig`] names secrets the way it
//! names anything else, by identifier, and the bytes arrive
//! separately in a [`Keyring`] the deployment fills from wherever
//! it actually keeps them: an environment variable, an encrypted
//! row, a secret manager. So a serialized config is safe to store
//! beside the rest of the deployment's configuration, and adding a
//! provider that needs two secrets, or twenty, does not change how
//! any of them travel.
//!
//! Elide ships one [`KeyProvider`] today, so [`KeyConfig`] has one
//! variant. Both are `#[non_exhaustive]`: a deployment that needs
//! per-label keys, a keyset with rotation, or a network-backed
//! store implements [`KeyProvider`] itself and hands the instance
//! to [`build_with_key_provider`], until a variant here covers it.
//!
//! [`KeyProvider`]: elide::redaction::operators::KeyProvider
//! [`build_with_key_provider`]: crate::ProviderConfig::build_with_key_provider

use std::collections::HashMap;
use std::sync::Arc;

use elide::redaction::operators::{KeyProvider, StaticKey};
use elide::{Error, ErrorKind, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The secrets a [`KeyConfig`] refers to, by name.
///
/// Filled by the deployment at startup and passed to
/// [`ProviderConfig::build`]; never serialized, and never part of a
/// config. Names are the deployment's own: whatever its config says
/// is what it must supply.
///
/// [`ProviderConfig::build`]: crate::ProviderConfig::build
#[derive(Default)]
pub struct Keyring {
    secrets: HashMap<String, Vec<u8>>,
}

impl Keyring {
    /// An empty keyring.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add `material` under `name`, replacing any secret already
    /// held there.
    #[must_use]
    pub fn with_secret(mut self, name: impl Into<String>, material: impl Into<Vec<u8>>) -> Self {
        self.secrets.insert(name.into(), material.into());
        self
    }

    /// Whether the keyring holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.secrets.is_empty()
    }

    /// The names this keyring holds, for reporting which of them a
    /// config never asked for.
    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.secrets.keys().map(String::as_str)
    }

    /// The secret named `name`.
    ///
    /// # Errors
    ///
    /// Returns [`Configuration`](ErrorKind::Configuration) when the
    /// keyring holds no secret under that name: the config asked
    /// for something the deployment did not supply, which is worth
    /// catching at startup rather than at the first request that
    /// needs a key.
    fn get(&self, name: &str) -> Result<&[u8]> {
        self.secrets.get(name).map(Vec::as_slice).ok_or_else(|| {
            Error::new(
                ErrorKind::Configuration,
                format!(
                    "the engine config names the secret `{name}`, which the keyring does not hold"
                ),
            )
        })
    }
}

/// Never prints secret names or material.
impl std::fmt::Debug for Keyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keyring").finish_non_exhaustive()
    }
}

/// Which key provider to build for the `HmacHash` and `Encrypt`
/// operators.
///
/// One provider backs both. A policy naming either operator without
/// one configured fails at request-compile time, naming the policy
/// and the operator, rather than redacting with some default key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[non_exhaustive]
pub enum KeyConfig {
    /// One deployment-wide key for every label.
    ///
    /// The common case, and elide's own [`StaticKey`]. Per-label
    /// keys need a [`KeyProvider`] of the deployment's own.
    #[serde(rename_all = "camelCase")]
    Static {
        /// The [`Keyring`] entry holding the key.
        secret: String,
    },
}

impl KeyConfig {
    /// Every keyring entry this config refers to.
    ///
    /// Lets the caller check the other direction: a secret the
    /// deployment supplied that no config names is a typo, and a
    /// typo in a secret name means redaction runs with the wrong
    /// key or none at all.
    pub(crate) fn secrets(&self) -> impl Iterator<Item = &str> {
        match self {
            Self::Static { secret } => std::iter::once(secret.as_str()),
        }
    }

    /// Build the provider this config names, reading whatever
    /// secrets it refers to out of `keyring`.
    ///
    /// # Errors
    ///
    /// Returns [`Configuration`](ErrorKind::Configuration) when the
    /// keyring does not hold a secret the config names.
    pub fn build(&self, keyring: &Keyring) -> Result<Arc<dyn KeyProvider>> {
        match self {
            Self::Static { secret } => {
                let material = keyring.get(secret)?;
                Ok(Arc::new(StaticKey::new(material.to_vec())))
            }
        }
    }
}
