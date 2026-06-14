//! Post-match validators for regex-detected entity values.
//!
//! A [`Variant`] inside a [`Regex`] rule may name a validator
//! (e.g. `validator: Some("luhn")`); the recognizer resolves the
//! name against a [`ValidatorRegistry`] at build time and drops
//! matches that fail the resolved check. Use validators to weed
//! out structurally-suspect false positives that a regex alone
//! can't.
//!
//! [`ValidatorRegistry::builtin`] ships with [`luhn`], [`iban`],
//! [`ssn`], [`phone`], and [`date`]. Each validator is also
//! re-exported as a free function so consumers can compose a
//! custom registry without taking the full set.
//!
//! [`Variant`]: crate::Variant
//! [`Regex`]: crate::Regex

mod date;
mod iban;
mod luhn;
mod phone;
mod ssn;

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

pub use self::date::date;
pub use self::iban::iban;
pub use self::luhn::luhn;
pub use self::phone::phone;
pub use self::ssn::ssn;

/// Post-match validator returning whether a matched string is
/// structurally valid.
///
/// Implemented by every `Fn(&str) -> bool + Send + Sync` via the
/// blanket impl, so plain function pointers slot in without a
/// wrapper type. Implement directly for types that need to carry
/// state (e.g. a remote-lookup client).
pub trait Validator: Send + Sync {
    /// Return `true` to keep the match, `false` to drop it.
    fn validate(&self, matched: &str) -> bool;
}

impl<F> Validator for F
where
    F: Fn(&str) -> bool + Send + Sync,
{
    fn validate(&self, matched: &str) -> bool {
        self(matched)
    }
}

/// Name → validator resolver consulted at recognizer-build time.
///
/// Keys are [`Cow<'static, str>`] so a `&'static str` literal stays
/// borrowed while a runtime-built name flows through as an owned
/// `String`.
#[derive(Clone, Default)]
pub struct ValidatorRegistry {
    table: HashMap<Cow<'static, str>, Arc<dyn Validator>>,
}

impl ValidatorRegistry {
    /// Construct an empty registry.
    ///
    /// Any [`Variant`] referencing a validator name will fail to
    /// resolve at recognizer-build time.
    ///
    /// [`Variant`]: crate::Variant
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Construct a registry pre-loaded with the built-in
    /// validators: [`luhn`], [`iban`], [`ssn`], [`phone`], [`date`].
    #[must_use]
    pub fn builtin() -> Self {
        Self::empty()
            .with("luhn", luhn)
            .with("iban", iban)
            .with("ssn", ssn)
            .with("phone", phone)
            .with("date", date)
    }

    /// Register `validator` under `name`, overwriting any previous
    /// entry with the same key.
    ///
    /// Override a built-in by registering under the same name
    /// (e.g. `"luhn"`).
    #[must_use]
    pub fn with<N, V>(mut self, name: N, validator: V) -> Self
    where
        N: Into<Cow<'static, str>>,
        V: Validator + 'static,
    {
        self.table.insert(name.into(), Arc::new(validator));
        self
    }

    /// Look up a validator by name.
    ///
    /// Returns `None` when the name is unregistered; the
    /// recognizer's build step surfaces that as a configuration
    /// error.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<Arc<dyn Validator>> {
        self.table.get(name).cloned()
    }
}

impl std::fmt::Debug for ValidatorRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.table.keys().map(AsRef::as_ref).collect();
        f.debug_struct("ValidatorRegistry")
            .field("validators", &names)
            .finish()
    }
}
