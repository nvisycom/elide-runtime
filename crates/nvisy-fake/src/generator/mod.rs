//! Per-[`EntityKind`] fake-value generation, dispatched by [`Locale`].
//!
//! [`generate`] returns `Some(string)` for every entity kind covered
//! by the core PII set, or `None` for kinds the fake-data layer
//! doesn't support — the caller substitutes a `[{entity_kind}]`
//! placeholder in that case.
//!
//! Each `fake` locale type implements a different per-faker trait
//! (`NameGenFn`, `CityNameGenFn`, …), so a generic dispatch helper
//! can't reach all 14 locales uniformly. Instead, every kind / locale
//! pair is enumerated below — explicit but verifiable.

use fake::Fake;
use fake::faker::address::raw as address;
use fake::faker::creditcard::raw as creditcard;
use fake::faker::currency::raw as currency;
use fake::faker::internet::raw as internet;
use fake::faker::name::raw as name;
use fake::faker::number::raw as number;
use fake::faker::phone_number::raw as phone_number;
use fake::locales::{
    AR_SA, CY_GB, DE_DE, EN, FA_IR, FR_FR, IT_IT, JA_JP, NL_NL, PT_BR, PT_PT, TR_TR, ZH_CN, ZH_TW,
};
use fake::rand::RngExt;
use nvisy_core::entity::EntityKind;

use crate::locale::Locale;

/// Generate a fake replacement string for `kind` in `locale`, using
/// `rng` as the entropy source. Returns `None` when the entity kind
/// isn't covered by the locale catalogue.
pub(crate) fn generate<R: RngExt + ?Sized>(
    locale: Locale,
    kind: EntityKind,
    rng: &mut R,
) -> Option<String> {
    let value: String = match (locale, kind) {
        (l, EntityKind::PersonName) => person_name(l, rng),
        (l, EntityKind::EmailAddress) => email(l, rng),
        (l, EntityKind::PhoneNumber) => phone(l, rng),
        (l, EntityKind::Address) => street_address(l, rng),
        (l, EntityKind::PostalCode) => postal_code(l, rng),
        (l, EntityKind::Url) => url(l, rng),
        (_, EntityKind::DateOfBirth) => date_of_birth(rng),
        (_, EntityKind::Age) => {
            let years: u8 = (1..=99u8).fake_with_rng(rng);
            years.to_string()
        }
        (l, EntityKind::PaymentCard) => credit_card(l, rng),
        (_, EntityKind::Iban) => iban(rng),
        (_, EntityKind::BankAccount) => bank_account(rng),
        (l, EntityKind::Currency) => currency_code(l, rng),
        _ => return None,
    };
    Some(value)
}

macro_rules! per_locale {
    ($locale:expr, $rng:expr, $faker:expr) => {
        match $locale {
            Locale::En => $faker(EN).fake_with_rng($rng),
            Locale::FrFr => $faker(FR_FR).fake_with_rng($rng),
            Locale::JaJp => $faker(JA_JP).fake_with_rng($rng),
            Locale::ZhCn => $faker(ZH_CN).fake_with_rng($rng),
            Locale::ZhTw => $faker(ZH_TW).fake_with_rng($rng),
            Locale::DeDe => $faker(DE_DE).fake_with_rng($rng),
            Locale::ItIt => $faker(IT_IT).fake_with_rng($rng),
            Locale::PtBr => $faker(PT_BR).fake_with_rng($rng),
            Locale::PtPt => $faker(PT_PT).fake_with_rng($rng),
            Locale::NlNl => $faker(NL_NL).fake_with_rng($rng),
            Locale::TrTr => $faker(TR_TR).fake_with_rng($rng),
            Locale::ArSa => $faker(AR_SA).fake_with_rng($rng),
            Locale::FaIr => $faker(FA_IR).fake_with_rng($rng),
            Locale::CyGb => $faker(CY_GB).fake_with_rng($rng),
        }
    };
}

fn person_name<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    per_locale!(locale, rng, name::Name)
}

fn email<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    per_locale!(locale, rng, internet::SafeEmail)
}

fn phone<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    per_locale!(locale, rng, phone_number::PhoneNumber)
}

fn postal_code<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    per_locale!(locale, rng, address::PostCode)
}

fn credit_card<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    per_locale!(locale, rng, creditcard::CreditCardNumber)
}

fn currency_code<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    per_locale!(locale, rng, currency::CurrencyCode)
}

fn street_address<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    let building: String = per_locale!(locale, rng, address::BuildingNumber);
    let street: String = per_locale!(locale, rng, address::StreetName);
    let city: String = per_locale!(locale, rng, address::CityName);
    format!("{building} {street}, {city}")
}

fn url<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    let user: String = per_locale!(locale, rng, internet::Username);
    let domain: String = per_locale!(locale, rng, internet::DomainSuffix);
    format!("https://www.{user}.{domain}")
}

/// Synthesise a YYYY-MM-DD date of birth without pulling in `chrono`:
/// pick a year in `[1940, 2010]`, a month in `[1, 12]`, and a day in
/// `[1, 28]` to stay valid in every month.
fn date_of_birth<R: RngExt + ?Sized>(rng: &mut R) -> String {
    let year: u16 = (1940..=2010u16).fake_with_rng(rng);
    let month: u8 = (1..=12u8).fake_with_rng(rng);
    let day: u8 = (1..=28u8).fake_with_rng(rng);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Synthesise an IBAN-shaped string: fixed `XX` country prefix plus
/// 22 digits. Looks IBAN-like without claiming any real check-digit
/// scheme.
fn iban<R: RngExt + ?Sized>(rng: &mut R) -> String {
    let body: String = number::NumberWithFormat(EN, "######################").fake_with_rng(rng);
    format!("XX{body}")
}

fn bank_account<R: RngExt + ?Sized>(rng: &mut R) -> String {
    number::NumberWithFormat(EN, "############").fake_with_rng(rng)
}
