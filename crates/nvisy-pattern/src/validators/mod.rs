//! Post-match validators for detected entity values.
//!
//! A [`Regex`] can reference a validator by name (e.g.
//! `validator: Some("luhn")`) to reduce false positives. At
//! [`PatternRecognizer::build`] time the name is resolved against a
//! [`ValidatorRegistry`] to a concrete validation function.
//!
//! The default [`ValidatorRegistry::builtin`] ships with five
//! validators — `luhn`, `iban`, `ssn`, `phone`, `date`. Consumers
//! can extend the registry with their own validators by calling
//! [`ValidatorRegistry::with`] before handing it to the recognizer
//! builder.
//!
//! [`Regex`]: crate::recognition::Regex
//! [`PatternRecognizer::build`]: crate::recognition::PatternRecognizer

mod date;
mod iban;
mod luhn;
mod phone;
mod ssn;

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

pub use self::date::validate_date;
pub use self::iban::validate_iban;
pub use self::luhn::luhn_check;
pub use self::phone::validate_phone;
pub use self::ssn::validate_ssn;

/// Post-match validator: returns `true` when `matched` passes the
/// validator's check.
///
/// Implemented by both built-in function-pointer validators (via the
/// blanket impl) and any third-party validator types a consumer
/// registers.
pub trait Validator: Send + Sync {
    /// Validate the text the recognizer matched. Returns `true` to
    /// keep the match, `false` to drop it.
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

/// Resolves validator names referenced in [`Regex`] definitions to
/// concrete [`Validator`] implementations.
///
/// Keys are [`Cow<'static, str>`] so the built-in registrations skip
/// any allocation (`&'static str` literal → borrowed variant) while
/// caller-supplied names that aren't `'static` (e.g. dynamically
/// constructed at runtime) still flow through as owned `String`s.
///
/// [`Regex`]: crate::recognition::Regex
#[derive(Clone, Default)]
pub struct ValidatorRegistry {
    table: HashMap<Cow<'static, str>, Arc<dyn Validator>>,
}

impl ValidatorRegistry {
    /// Empty registry — no validators registered. Regex rules that
    /// reference a validator name will fail to resolve at recognizer
    /// build time.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Registry pre-loaded with every built-in validator: `luhn`,
    /// `iban`, `ssn`, `phone`, `date`.
    #[must_use]
    pub fn builtin() -> Self {
        Self::empty()
            .with("luhn", luhn_check)
            .with("iban", validate_iban)
            .with("ssn", validate_ssn)
            .with("phone", validate_phone)
            .with("date", validate_date)
    }

    /// Register `validator` under `name`. Overwrites any previous
    /// entry with the same name.
    ///
    /// Built-ins live under `"luhn"`, `"iban"`, `"ssn"`, `"phone"`,
    /// and `"date"`; consumers can override them with their own
    /// implementations by registering under the same name.
    ///
    /// `name` accepts anything convertible to [`Cow<'static, str>`]
    /// — a `&'static str` literal stays borrowed (zero allocation),
    /// an owned `String` becomes the owned variant.
    #[must_use]
    pub fn with<N, V>(mut self, name: N, validator: V) -> Self
    where
        N: Into<Cow<'static, str>>,
        V: Validator + 'static,
    {
        self.table.insert(name.into(), Arc::new(validator));
        self
    }

    /// Look up a validator by name, returning the registered
    /// implementation or `None` when the name is unknown.
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
