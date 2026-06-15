//! Post-match validators for regex-detected entity values.
//!
//! A [`Variant`] inside a [`Regex`] rule may name a validator
//! (e.g. `validator: Some("luhn")`); the recognizer resolves the
//! name against a [`ValidatorRegistry`] at build time and drops
//! matches that fail the resolved check. Use validators to weed
//! out structurally-suspect false positives that a regex alone
//! can't.
//!
//! [`ValidatorRegistry::builtin`] ships universal validators
//! ([`luhn`], [`iban`], [`phone`], [`date`]) plus jurisdiction-
//! scoped sets re-exported from [`us`] (`"us.ssn"`,
//! `"us.aba_routing"`, `"us.npi"`, `"us.dea_number"`) and [`uk`]
//! (`"uk.nhs"`, `"uk.nino"`). Each validator is also re-exported
//! as a free function so consumers can compose a custom registry
//! without taking the full set.
//!
//! [`Variant`]: crate::Variant
//! [`Regex`]: crate::Regex

mod date;
mod iban;
mod luhn;
mod phone;

pub mod uk;
pub mod us;

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

pub use self::date::date;
pub use self::iban::iban;
pub use self::luhn::luhn;
pub use self::phone::phone;

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

    /// Construct a registry pre-loaded with the shipped built-in
    /// validators.
    ///
    /// Universal keys: `"luhn"`, `"iban"`, `"phone"`, `"date"`.
    ///
    /// US-scoped: `"us.ssn"`, `"us.aba_routing"`, `"us.npi"`,
    /// `"us.dea_number"`.
    ///
    /// UK-scoped: `"uk.nhs"`, `"uk.nino"`.
    #[must_use]
    pub fn builtin() -> Self {
        Self::empty()
            .with("luhn", luhn)
            .with("iban", iban)
            .with("phone", phone)
            .with("date", date)
            .with("us.ssn", us::ssn)
            .with("us.aba_routing", us::aba_routing)
            .with("us.npi", us::npi)
            .with("us.dea_number", us::dea_number)
            .with("uk.nhs", uk::nhs)
            .with("uk.nino", uk::nino)
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
