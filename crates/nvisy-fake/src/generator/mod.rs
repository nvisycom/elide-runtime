//! Per-[`EntityKind`] fake-value generation, dispatched by [`Locale`].
//!
//! [`generate`] returns `Some(string)` for every entity kind covered
//! by the core PII set, or `None` for kinds the fake-data layer
//! doesn't support — the caller substitutes a `[{entity_kind}]`
//! placeholder in that case.
//!
//! Each `fake` locale exposes a different per-faker trait gate, so
//! all per-locale dispatch goes through the [`dispatch!`] macro
//! defined in [`dispatch`].
//!
//! [`dispatch!`]: dispatch::dispatch

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

use self::dispatch::dispatch;
use crate::anonymizer::{honors_format, is_fixed_width};
use crate::locale::Locale;

/// Per-call options threaded through to each kind generator.
pub(crate) struct Context<'a> {
    pub locale: Locale,
    pub kind: EntityKind,
    pub length_preserving: bool,
    pub format_preserving: bool,
    pub original: &'a str,
}

/// Generate a fake replacement string for `ctx.kind` in `ctx.locale`,
/// using `rng` as the entropy source. Returns `None` when the entity
/// kind isn't covered by the locale catalogue.
pub(crate) fn generate<R: RngExt + ?Sized>(ctx: Context<'_>, rng: &mut R) -> Option<String> {
    let raw = produce(&ctx, rng)?;
    Some(post_process(&ctx, raw))
}

fn produce<R: RngExt + ?Sized>(ctx: &Context<'_>, rng: &mut R) -> Option<String> {
    let value = match ctx.kind {
        EntityKind::PersonName => dispatch!(ctx.locale, rng, name::Name),
        EntityKind::EmailAddress => dispatch!(ctx.locale, rng, internet::SafeEmail),
        EntityKind::PhoneNumber => dispatch!(ctx.locale, rng, phone_number::PhoneNumber),
        EntityKind::Address => address::street_address(ctx.locale, rng),
        EntityKind::PostalCode => dispatch!(ctx.locale, rng, address_raw::PostCode),
        EntityKind::Url => url::url(ctx.locale, rng),
        EntityKind::DateOfBirth => dob::date_of_birth(ctx.locale, rng),
        EntityKind::Age => {
            let years: u8 = (1..=99u8).fake_with_rng(rng);
            years.to_string()
        }
        EntityKind::PaymentCard => dispatch!(ctx.locale, rng, creditcard::CreditCardNumber),
        EntityKind::Iban => iban::iban(ctx.locale, rng),
        EntityKind::BankAccount => bank::bank_account(ctx.locale, rng),
        EntityKind::Currency => dispatch!(ctx.locale, rng, currency::CurrencyCode),
        _ => return None,
    };
    Some(value)
}

fn post_process(ctx: &Context<'_>, mut value: String) -> String {
    if ctx.format_preserving && honors_format(ctx.kind) && !ctx.original.is_empty() {
        value = format::reshape_to_original(&value, ctx.original);
    }
    if ctx.length_preserving && is_fixed_width(ctx.kind) && !ctx.original.is_empty() {
        value = format::clip_or_pad(&value, ctx.original.chars().count());
    }
    value
}

/// Shared helper for kinds that synthesise digit groups outside the
/// fake-rs locale tables (IBAN, bank account, DOB).
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
        Context {
            locale,
            kind,
            length_preserving: false,
            format_preserving: false,
            original,
        }
    }

    #[test]
    fn unsupported_kind_returns_none() {
        let mut rng = rng();
        assert!(generate(ctx(Locale::En, EntityKind::IpAddress, ""), &mut rng).is_none());
    }

    #[test]
    fn supported_kinds_return_non_empty() {
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
            let out = generate(ctx(Locale::En, kind, ""), &mut rng)
                .unwrap_or_else(|| panic!("no value for {kind:?}"));
            assert!(!out.is_empty(), "empty for {kind:?}");
        }
    }
}
