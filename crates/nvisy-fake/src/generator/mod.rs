//! Per-[`EntityKind`] fake-value generation, dispatched by [`Locale`].
//!
//! [`generate`] returns `Some(string)` for every entity kind covered
//! by the core PII set, or `None` for kinds the fake-data layer
//! doesn't support — the caller substitutes a `[{entity_kind}]`
//! placeholder in that case.

mod address;
mod bank;
mod dispatch;
mod dob;
mod format;
mod iban;
mod url;

use fake::Fake;
use fake::faker::address::raw as address_raw;
use fake::faker::creditcard::raw as creditcard;
use fake::faker::currency::raw as currency;
use fake::faker::internet::raw as internet;
use fake::faker::name::raw as name;
use fake::faker::number::raw as number;
use fake::faker::phone_number::raw as phone_number;
use fake::locales::EN;
use fake::rand::RngExt;
use nvisy_core::entity::EntityKind;

use self::dispatch::fan_locale;
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
    /// kind isn't covered by the locale catalogue.
    pub(crate) fn generate<R: RngExt + ?Sized>(self, rng: &mut R) -> Option<String> {
        let raw = self.produce(rng)?;
        Some(self.post_process(raw))
    }

    fn produce<R: RngExt + ?Sized>(&self, rng: &mut R) -> Option<String> {
        let value = match self.kind {
            EntityKind::PersonName => fan_locale!(self.locale, rng, name::Name),
            EntityKind::EmailAddress => fan_locale!(self.locale, rng, internet::SafeEmail),
            EntityKind::PhoneNumber => fan_locale!(self.locale, rng, phone_number::PhoneNumber),
            EntityKind::Address => address::street_address(self.locale, rng),
            EntityKind::PostalCode => fan_locale!(self.locale, rng, address_raw::PostCode),
            EntityKind::Url => url::url(self.locale, rng),
            EntityKind::DateOfBirth => dob::date_of_birth(self.locale, rng),
            EntityKind::Age => {
                let years: u8 = (1..=99u8).fake_with_rng(rng);
                years.to_string()
            }
            EntityKind::PaymentCard => fan_locale!(self.locale, rng, creditcard::CreditCardNumber),
            EntityKind::Iban => iban::iban(self.locale, rng)?,
            EntityKind::BankAccount => bank::bank_account(self.locale, rng),
            EntityKind::Currency => fan_locale!(self.locale, rng, currency::CurrencyCode),
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
/// fake-rs locale tables (IBAN, bank account, DOB).
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
        EntityKind::PaymentCard | EntityKind::Iban | EntityKind::PostalCode
    )
}

/// Kinds that honor the format-preserving toggle. Both reshape into
/// a digit-shape with separators borrowed from the original span.
fn honors_format(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::PhoneNumber | EntityKind::PostalCode)
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
    fn unsupported_kind_returns_none() {
        let mut rng = rng();
        assert!(
            ctx(Locale::En, EntityKind::IpAddress, "")
                .generate(&mut rng)
                .is_none()
        );
    }

    #[test]
    fn supported_kinds_return_non_empty() {
        // Use a locale with a national IBAN scheme so `Iban`
        // doesn't fall through to None (En, JaJp, ZhCn/Tw, FaIr
        // intentionally lack one).
        let kinds = [
            EntityKind::PersonName,
            EntityKind::EmailAddress,
            EntityKind::PhoneNumber,
            EntityKind::Address,
            EntityKind::PostalCode,
            EntityKind::Url,
            EntityKind::DateOfBirth,
            EntityKind::Age,
            EntityKind::PaymentCard,
            EntityKind::Iban,
            EntityKind::BankAccount,
            EntityKind::Currency,
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
