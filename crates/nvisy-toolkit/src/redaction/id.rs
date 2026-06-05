//! [`AnonymizerId<M>`]: the lookup handle for a custom
//! [`Anonymizer<M>`] in the [`RedactionRegistry<M>`].
//!
//! Identifies a deployment-registered custom operator by name. The
//! `M` phantom binds the id to a single modality at the type level —
//! a `AnonymizerId<Text>` cannot be handed to the image registry.
//!
//! The built-in operators bundled with the toolkit
//! ([`Replace`], [`Mask`], [`Hash`], [`Redact`],
//! [`Keep`], [`Encrypt`]) do **not** carry an `AnonymizerId` —
//! they are instantiated per-call from policy-supplied params and
//! never round-trip through the registry. `AnonymizerId` exists
//! solely for the `Custom` escape hatch used to plug stateful Rust
//! operators (KMS clients, token vaults, …) that can't be expressed
//! as a config blob.
//!
//! [`Anonymizer<M>`]: super::Anonymizer
//! [`RedactionRegistry<M>`]: super::RedactionRegistry
//! [`Replace`]: super::builtin::Replace
//! [`Mask`]: super::builtin::Mask
//! [`Hash`]: super::builtin::Hash
//! [`Redact`]: super::builtin::Redact
//! [`Keep`]: super::builtin::Keep
//! [`Encrypt`]: super::builtin::Encrypt

use std::borrow::Cow;
use std::fmt::{self, Debug, Display};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Redactable;

/// Lookup handle for a custom [`Anonymizer<M>`] in the
/// [`RedactionRegistry<M>`]. Phantom-typed over `M` so the compiler
/// rejects cross-modality use at the call site.
///
/// Ids are author-supplied strings (deployment chooses the
/// vocabulary). Equality and hashing ignore the modality marker —
/// the name is the actual key.
///
/// [`Anonymizer<M>`]: super::Anonymizer
/// [`RedactionRegistry<M>`]: super::RedactionRegistry
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct AnonymizerId<M: Redactable> {
    name: Cow<'static, str>,
    #[serde(skip)]
    #[schemars(skip)]
    _modality: PhantomData<fn() -> M>,
}

impl<M: Redactable> AnonymizerId<M> {
    /// Build an id from a `&'static str` without allocating.
    #[must_use]
    pub const fn from_static(name: &'static str) -> Self {
        Self {
            name: Cow::Borrowed(name),
            _modality: PhantomData,
        }
    }

    /// Build an id from any string-like value.
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            name: name.into(),
            _modality: PhantomData,
        }
    }

    /// Borrow the underlying name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.name
    }
}

impl<M: Redactable> Clone for AnonymizerId<M> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            _modality: PhantomData,
        }
    }
}

impl<M: Redactable> Debug for AnonymizerId<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AnonymizerId").field(&self.name).finish()
    }
}

impl<M: Redactable> Display for AnonymizerId<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.name, f)
    }
}

impl<M: Redactable> PartialEq for AnonymizerId<M> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl<M: Redactable> Eq for AnonymizerId<M> {}

impl<M: Redactable> Hash for AnonymizerId<M> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl<M: Redactable> From<&'static str> for AnonymizerId<M> {
    fn from(name: &'static str) -> Self {
        Self::from_static(name)
    }
}

impl<M: Redactable> From<String> for AnonymizerId<M> {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}
