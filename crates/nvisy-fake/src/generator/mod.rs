//! Per-[`EntityKind`] fake-value generation, dispatched by [`Locale`].
//!
//! [`Context::generate`] returns `Some(string)` for every entity
//! kind the catalogue covers, or `None` for kinds the fake-data
//! layer doesn't support — the caller delegates to its fallback
//! anonymizer in that case.
//!
//! Generators are grouped by domain:
//! - [`identity`] — names, organisations, jobs, categorical labels
//! - [`finance`] — payment, banking, IBAN, currency, amounts
//! - [`contact`] — addresses, phones, emails, URLs, coordinates,
//!   licence plates
//! - [`device`] — IP/MAC, passwords, API tokens, device UUIDs
//! - [`temporal`] — date of birth, datetime, age
//! - [`case_id`] — opaque numeric identifiers

mod case_id;
mod contact;
mod device;
mod dispatch;
mod finance;
mod format;
mod identity;
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
    length_preserving: bool,
    format_preserving: bool,
    original: &'a str,
}

impl<'a> Context<'a> {
    /// Build a generation request.
    pub(crate) fn new(
        locale: Locale,
        kind: EntityKind,
        length_preserving: bool,
        format_preserving: bool,
        original: &'a str,
    ) -> Self {
        Self {
            locale,
            kind,
            length_preserving,
            format_preserving,
            original,
        }
    }

    /// Generate a fake replacement string for this context, using
    /// `rng` as the entropy source. Returns `None` when the entity
    /// kind isn't covered.
    pub(crate) fn generate<R: RngExt + ?Sized>(self, rng: &mut R) -> Option<String> {
        let raw = self.produce(rng)?;
        Some(self.post_process(raw))
    }

    fn produce<R: RngExt + ?Sized>(&self, rng: &mut R) -> Option<String> {
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

            // contact
            EntityKind::EmailAddress => contact::email(l, rng),
            EntityKind::PhoneNumber => contact::phone(l, rng),
            EntityKind::Address => contact::street_address(l, rng),
            EntityKind::PostalCode => contact::postal_code(l, rng),
            EntityKind::Url => contact::url(l, rng),
            EntityKind::Coordinates => contact::coordinates(l, rng),
            EntityKind::LicensePlate => contact::license_plate(l, rng),

            // device
            EntityKind::IpAddress => device::ip_address(l, rng),
            EntityKind::MacAddress => device::mac_address(l, rng),
            EntityKind::Password => device::password(l, rng),
            EntityKind::ApiKey => device::api_key(rng),
            EntityKind::AuthToken => device::auth_token(rng),
            EntityKind::DeviceId => device::device_id(rng),

            // temporal
            EntityKind::DateOfBirth => temporal::date_of_birth(l, rng),
            EntityKind::DateTime => temporal::date_time(l, rng),
            EntityKind::Age => temporal::age(rng),

            // finance
            EntityKind::PaymentCard => finance::payment_card(l, rng),
            EntityKind::CardSecurityCode => finance::card_security_code(rng),
            EntityKind::CardExpiry => finance::card_expiry(l, rng),
            EntityKind::Iban => finance::iban(l, rng)?,
            EntityKind::BankAccount => finance::bank_account(l, rng),
            EntityKind::BankRouting => finance::bank_routing(rng),
            EntityKind::SwiftCode => finance::swift_code(l, rng),
            EntityKind::Currency => finance::currency_code(l, rng),
            EntityKind::Amount => finance::amount(l, rng),
            EntityKind::Quantity => finance::quantity(rng),

            // case ids
            EntityKind::InternalId | EntityKind::CaseNumber => case_id::internal_id(rng),

            _ => return None,
        };
        Some(value)
    }

    fn post_process(&self, mut value: String) -> String {
        if self.format_preserving && honors_format(self.kind) && !self.original.is_empty() {
            value = format::reshape_to_original(&value, self.original);
        }
        if self.length_preserving && is_fixed_width(self.kind) && !self.original.is_empty() {
            value = format::clip_or_pad(&value, self.original.chars().count());
        }
        value
    }
}

/// Shared helper for kinds that synthesise digit groups outside the
/// fake-rs locale tables (IBAN, bank account, IDs).
pub(crate) fn digits<R: RngExt + ?Sized>(len: usize, rng: &mut R) -> String {
    let fmt = "#".repeat(len);
    number::NumberWithFormat(EN, fmt.as_str()).fake_with_rng(rng)
}

/// Kinds that honor the length-preserving toggle. All are
/// fixed-width ASCII digit strings, which is what makes
/// `clip_or_pad`'s `'0'`-padding safe — padding into multibyte
/// scripts would not be.
fn is_fixed_width(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::PaymentCard
            | EntityKind::Iban
            | EntityKind::PostalCode
            | EntityKind::CardSecurityCode
            | EntityKind::BankRouting
    )
}

/// Kinds that honor the format-preserving toggle. All reshape into
/// a digit-shape with separators borrowed from the original span.
fn honors_format(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::PhoneNumber | EntityKind::PostalCode | EntityKind::CardExpiry
    )
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
        Context::new(locale, kind, false, false, original)
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
    fn supported_kinds_return_non_empty() {
        let kinds = [
            // identity
            EntityKind::PersonName,
            EntityKind::OrganizationName,
            EntityKind::Occupation,
            EntityKind::Username,
            EntityKind::Gender,
            EntityKind::Language,
            EntityKind::Nationality,
            EntityKind::Citizenship,
            // contact
            EntityKind::EmailAddress,
            EntityKind::PhoneNumber,
            EntityKind::Address,
            EntityKind::PostalCode,
            EntityKind::Url,
            EntityKind::Coordinates,
            EntityKind::LicensePlate,
            // device
            EntityKind::IpAddress,
            EntityKind::MacAddress,
            EntityKind::Password,
            EntityKind::ApiKey,
            EntityKind::AuthToken,
            EntityKind::DeviceId,
            // temporal
            EntityKind::DateOfBirth,
            EntityKind::DateTime,
            EntityKind::Age,
            // finance
            EntityKind::PaymentCard,
            EntityKind::CardSecurityCode,
            EntityKind::CardExpiry,
            EntityKind::Iban,
            EntityKind::BankAccount,
            EntityKind::BankRouting,
            EntityKind::SwiftCode,
            EntityKind::Currency,
            EntityKind::Amount,
            EntityKind::Quantity,
            // case ids
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

    #[test]
    fn iban_falls_through_for_non_iban_locales() {
        let mut rng = rng();
        assert!(
            ctx(Locale::JaJp, EntityKind::Iban, "")
                .generate(&mut rng)
                .is_none()
        );
        assert!(
            ctx(Locale::En, EntityKind::Iban, "")
                .generate(&mut rng)
                .is_none()
        );
    }
}
