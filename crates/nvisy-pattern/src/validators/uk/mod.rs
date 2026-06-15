//! UK-specific post-match validators.
//!
//! Registered under the [`ValidatorRegistry::builtin`] set with
//! dotted names — `"uk.nhs"`, `"uk.nino"`.
//!
//! [`ValidatorRegistry::builtin`]: super::ValidatorRegistry::builtin

mod nhs;
mod nino;

pub use self::nhs::nhs;
pub use self::nino::nino;
