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
//! ([`luhn`], [`iban`], [`phone`], [`date`], [`btc`]) plus
//! jurisdiction-scoped sets re-exported from [`us`] (`"us.ssn"`,
//! `"us.aba_routing"`, `"us.npi"`, `"us.dea_number"`,
//! `"us.postal_code"`) and [`uk`]
//! (`"uk.nhs"`, `"uk.nino"`, `"uk.driving_licence"`,
//! `"uk.vehicle_registration"`). Each validator is also re-exported
//! as a free function so consumers can compose a custom registry
//! without taking the full set.
//!
//! [`Variant`]: crate::Variant
//! [`Regex`]: crate::Regex

mod btc;
mod date;
mod iban;
mod luhn;
mod phone;

pub mod uk;
pub mod us;

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use nvisy_core::primitive::{CountryCode, LanguageTag};

pub use self::btc::btc;
pub use self::date::date;
pub use self::iban::iban;
pub use self::luhn::luhn;
pub use self::phone::phone;

/// Per-call hints supplied to validators alongside the matched
/// string.
///
/// Carries the caller's [`RecognizerInput`] jurisdiction and
/// language so validators that need region-aware semantics
/// (e.g. `phone`) can honour the caller's intent instead of
/// guessing across a fixed fallback set. Validators that don't
/// need either field can ignore it via `_ctx`.
///
/// [`RecognizerInput`]: crate::recognition::RecognizerInput
#[derive(Debug, Clone, Default)]
pub struct ValidationContext {
    /// ISO 3166-1 alpha-2 jurisdiction associated with the input,
    /// when the caller specified one.
    pub country: Option<CountryCode>,
    /// BCP-47 language tag associated with the input, when the
    /// caller specified one.
    pub language: Option<LanguageTag>,
}

/// Post-match validator returning whether a matched string is
/// structurally valid.
///
/// Implemented by every `Fn(&str, &ValidationContext) -> bool +
/// Send + Sync` via the blanket impl, so plain function pointers
/// slot in without a wrapper type. Implement directly for types
/// that need to carry state (e.g. a remote-lookup client).
pub trait Validator: Send + Sync {
    /// Return `true` to keep the match, `false` to drop it.
    fn validate(&self, matched: &str, ctx: &ValidationContext) -> bool;
}

impl<F> Validator for F
where
    F: Fn(&str, &ValidationContext) -> bool + Send + Sync,
{
    fn validate(&self, matched: &str, ctx: &ValidationContext) -> bool {
        self(matched, ctx)
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
    /// Universal keys: `"luhn"`, `"iban"`, `"phone"`, `"date"`,
    /// `"crypto.btc"`.
    ///
    /// US-scoped: `"us.ssn"`, `"us.aba_routing"`, `"us.npi"`,
    /// `"us.dea_number"`, `"us.postal_code"`.
    ///
    /// UK-scoped: `"uk.nhs"`, `"uk.nino"`,
    /// `"uk.driving_licence"`, `"uk.vehicle_registration"`.
    #[must_use]
    pub fn builtin() -> Self {
        Self::empty()
            .with_simple("luhn", luhn)
            .with_simple("iban", iban)
            .with("phone", phone)
            .with_simple("date", date)
            .with_simple("crypto.btc", btc)
            .with_simple("us.ssn", us::ssn)
            .with_simple("us.aba_routing", us::aba_routing)
            .with_simple("us.npi", us::npi)
            .with_simple("us.dea_number", us::dea_number)
            .with_simple("us.postal_code", us::postal_code)
            .with_simple("uk.nhs", uk::nhs)
            .with_simple("uk.nino", uk::nino)
            .with_simple("uk.driving_licence", uk::driving_licence)
            .with_simple("uk.vehicle_registration", uk::vehicle_registration)
    }

    /// Register a context-aware `validator` under `name`,
    /// overwriting any previous entry with the same key.
    ///
    /// Override a built-in by registering under the same name
    /// (e.g. `"phone"`).
    #[must_use]
    pub fn with<N, V>(mut self, name: N, validator: V) -> Self
    where
        N: Into<Cow<'static, str>>,
        V: Validator + 'static,
    {
        self.table.insert(name.into(), Arc::new(validator));
        self
    }

    /// Register a context-free `Fn(&str) -> bool` validator under
    /// `name`. Convenience wrapper around [`Self::with`] for the
    /// common case where the validator ignores
    /// [`ValidationContext`].
    #[must_use]
    pub fn with_simple<N, F>(self, name: N, validator: F) -> Self
    where
        N: Into<Cow<'static, str>>,
        F: Fn(&str) -> bool + Send + Sync + 'static,
    {
        self.with(name, move |s: &str, _: &ValidationContext| validator(s))
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
