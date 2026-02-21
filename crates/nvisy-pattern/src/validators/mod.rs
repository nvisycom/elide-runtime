//! Post-match validators for detected entity values.
//!
//! Patterns can reference a validator by name (e.g. `"validator": "luhn"`)
//! to reduce false positives.  At detection time the name is resolved to a
//! [`ValidatorFn`] via [`ValidatorResolver`].
//!
//! # Built-in validators
//!
//! | Name   | Module | Description                                |
//! |--------|--------|--------------------------------------------|
//! | `ssn`  | `ssn`  | US Social Security Number format check     |
//! | `luhn` | `luhn` | Luhn checksum for credit card numbers      |

mod luhn;
mod ssn;

pub use luhn::luhn_check;
pub use ssn::validate_ssn;

use std::collections::HashMap;

/// Signature for a validation function: takes the matched text and returns
/// `true` if the value is valid.
pub type ValidatorFn = fn(&str) -> bool;

/// Maps validator names to [`ValidatorFn`]s.
///
/// Created with the built-in validators via [`builtins`] (or
/// [`Default`]), then optionally extended with [`register`] for
/// custom validators.
///
/// [`builtins`]: Self::builtins
/// [`register`]: Self::register
#[derive(Debug, Clone)]
pub struct ValidatorResolver {
    table: HashMap<&'static str, ValidatorFn>,
}

impl ValidatorResolver {
    /// Create a resolver pre-loaded with all built-in validators.
    pub fn builtins() -> Self {
        let mut r = Self {
            table: HashMap::new(),
        };
        r.register("ssn", validate_ssn);
        r.register("luhn", luhn_check);
        r
    }

    /// Register a validator function under the given name.
    ///
    /// Overwrites any previously registered validator with the same name.
    pub fn register(&mut self, name: &'static str, f: ValidatorFn) {
        self.table.insert(name, f);
    }

    /// Look up a validator by name, returning `None` if unregistered.
    pub fn resolve(&self, name: &str) -> Option<ValidatorFn> {
        self.table.get(name).copied()
    }
}

impl Default for ValidatorResolver {
    fn default() -> Self {
        Self::builtins()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_builtins() {
        let r = ValidatorResolver::builtins();
        assert!(r.resolve("ssn").is_some());
        assert!(r.resolve("luhn").is_some());
        assert!(r.resolve("nope").is_none());
    }

    #[test]
    fn resolver_custom() {
        let mut r = ValidatorResolver::builtins();
        r.register("always_true", |_| true);
        assert!(r.resolve("always_true").unwrap()("anything"));
    }
}
