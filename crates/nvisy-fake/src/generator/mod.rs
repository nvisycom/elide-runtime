//! Per-[`EntityKind`] fake-value generation, dispatched by [`Locale`].
//!
//! [`Context::generate`] returns `Some(string)` for every entity
//! kind the catalogue covers, or `None` for kinds the fake-data
//! layer doesn't support — the caller delegates to its fallback
//! anonymizer in that case.
//!
//! Two paths:
//!
//! - **Structured kinds** (IBAN, payment cards, dates, IPs, …)
//!   pattern-preserve the original string: same length, same
//!   character-class layout, randomised digits and letters.
//!   See [`pattern::pattern_preserve`].
//! - **Free-form kinds** (names, addresses, organisations, …)
//!   emit a fresh locale-aware fake whose length doesn't need to
//!   match. These go through per-domain submodules.

mod case_id;
mod contact;
mod device;
mod dispatch;
mod finance;
mod identity;
mod pattern;
mod temporal;

use fake::Fake;
use fake::faker::number::raw as number;
use fake::locales::EN;
use fake::rand::RngExt;
use nvisy_core::entity::EntityKind;

use crate::locale::Locale;

/// Per-call options threaded through to each kind generator.
pub(crate) struct Context<'a> {
    locale: Locale,
    kind: EntityKind,
    original: &'a str,
}

impl<'a> Context<'a> {
    /// Build a generation request.
    pub(crate) fn new(locale: Locale, kind: EntityKind, original: &'a str) -> Self {
        Self {
            locale,
            kind,
            original,
        }
    }

    /// Generate a fake replacement string for this context, using
    /// `rng` as the entropy source. Returns `None` when the entity
    /// kind isn't covered.
    pub(crate) fn generate<R: RngExt + ?Sized>(self, rng: &mut R) -> Option<String> {
        // Structured kinds: scramble the original in place. Skip
        // when source is empty — there's no pattern to copy from.
        if self.kind.is_structured() {
            if self.original.is_empty() {
                return None;
            }
            return Some(pattern::pattern_preserve(self.original, rng));
        }
        // Free-form kinds: locale-aware generator.
        self.produce_free_form(rng)
    }

    fn produce_free_form<R: RngExt + ?Sized>(&self, rng: &mut R) -> Option<String> {
        let l = self.locale;
        let value = match self.kind {
            // identity
            EntityKind::PersonName => identity::person_name(l, rng),
            EntityKind::OrganizationName => identity::organization_name(l, rng),
            EntityKind::Occupation => identity::occupation(l, rng),
            EntityKind::Username => identity::username(l, rng),
            EntityKind::Gender => identity::gender(l, rng),
            EntityKind::Language => identity::language(rng),
            EntityKind::Nationality => identity::nationality(l, rng),
            EntityKind::Citizenship => identity::citizenship(l, rng),

            // contact (free-form subset)
            EntityKind::Address => contact::street_address(l, rng),
            EntityKind::Url => contact::url(l, rng),

            // temporal (only Age is free-form; DateOfBirth/DateTime
            // are structured and pattern-preserved above).
            EntityKind::Age => temporal::age(rng),

            // finance (free-form subset)
            EntityKind::Currency => finance::currency_code(l, rng),
            EntityKind::Amount => finance::amount(l, rng),
            EntityKind::Quantity => finance::quantity(rng),

            // device (free-form subset: random tokens)
            EntityKind::Password => device::password(l, rng),
            EntityKind::ApiKey => device::api_key(rng),
            EntityKind::AuthToken => device::auth_token(rng),
            EntityKind::DeviceId => device::device_id(rng),

            // case ids
            EntityKind::InternalId | EntityKind::CaseNumber => case_id::internal_id(rng),

            _ => return None,
        };
        Some(value)
    }
}

/// Shared helper for kinds that synthesise digit groups outside
/// the fake-rs locale tables (bank account, IDs).
pub(crate) fn digits<R: RngExt + ?Sized>(len: usize, rng: &mut R) -> String {
    let fmt = "#".repeat(len);
    number::NumberWithFormat(EN, fmt.as_str()).fake_with_rng(rng)
}

#[cfg(test)]
mod tests {
    use fake::rand::SeedableRng;
    use fake::rand::rngs::SmallRng;

    use super::*;

    fn rng() -> SmallRng {
        SmallRng::seed_from_u64(7)
    }

    fn ctx<'a>(locale: Locale, kind: EntityKind, original: &'a str) -> Context<'a> {
        Context::new(locale, kind, original)
    }

    #[test]
    fn unsupported_kinds_return_none() {
        let mut rng = rng();
        for kind in [
            EntityKind::Fingerprint,
            EntityKind::Face,
            EntityKind::Religion,
            EntityKind::Diagnosis,
        ] {
            assert!(
                ctx(Locale::En, kind, "").generate(&mut rng).is_none(),
                "{kind:?} should be None"
            );
        }
    }

    #[test]
    fn structured_kind_with_empty_source_returns_none() {
        let mut rng = rng();
        // No pattern to copy → can't pattern-preserve.
        assert!(
            ctx(Locale::En, EntityKind::Iban, "")
                .generate(&mut rng)
                .is_none()
        );
    }

    #[test]
    fn structured_kinds_preserve_original_shape() {
        let cases: &[(EntityKind, &str)] = &[
            (EntityKind::Iban, "GB82WEST12345698765432"),
            (EntityKind::PaymentCard, "4111-1111-1111-1111"),
            (EntityKind::PhoneNumber, "+1-555-123-4567"),
            (EntityKind::DateOfBirth, "1985-03-12"),
            (EntityKind::IpAddress, "192.168.1.1"),
            (EntityKind::PostalCode, "SW1A 1AA"),
        ];
        for &(kind, original) in cases {
            let mut rng = rng();
            let out = ctx(Locale::En, kind, original).generate(&mut rng).unwrap();
            assert_eq!(out.len(), original.len(), "{kind:?}: length mismatch");
            // Separator positions match.
            for (i, (a, b)) in out.chars().zip(original.chars()).enumerate() {
                if !a.is_ascii_alphanumeric() {
                    assert_eq!(a, b, "{kind:?}: separator mismatch at {i} ({a:?} vs {b:?})");
                }
            }
        }
    }

    #[test]
    fn free_form_kinds_return_non_empty() {
        let kinds = [
            EntityKind::PersonName,
            EntityKind::OrganizationName,
            EntityKind::Occupation,
            EntityKind::Username,
            EntityKind::Gender,
            EntityKind::Language,
            EntityKind::Nationality,
            EntityKind::Citizenship,
            EntityKind::Address,
            EntityKind::Url,
            EntityKind::Age,
            EntityKind::Currency,
            EntityKind::Amount,
            EntityKind::Quantity,
            EntityKind::Password,
            EntityKind::ApiKey,
            EntityKind::AuthToken,
            EntityKind::DeviceId,
            EntityKind::InternalId,
            EntityKind::CaseNumber,
        ];
        for kind in kinds {
            let mut rng = rng();
            let out = ctx(Locale::DeDe, kind, "")
                .generate(&mut rng)
                .unwrap_or_else(|| panic!("no value for {kind:?}"));
            assert!(!out.is_empty(), "empty for {kind:?}");
        }
    }
}
