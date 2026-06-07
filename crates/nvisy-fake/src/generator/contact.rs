//! Physical and network locators: addresses, postal codes, phones,
//! emails, URLs, coordinates, vehicle plates.

use fake::Fake;
use fake::faker::address::raw as address;
use fake::faker::automotive::raw as automotive;
use fake::faker::internet::raw as internet;
use fake::faker::phone_number::raw as phone_number;
use fake::locales::{FA_IR, FR_FR, IT_IT, NL_NL, PT_PT, TR_TR};
use fake::rand::RngExt;

use super::digits;
use super::dispatch::fan_locale;
use crate::locale::Locale;

pub(super) fn email<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    fan_locale!(locale, rng, internet::SafeEmail)
}

pub(super) fn phone<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    fan_locale!(locale, rng, phone_number::PhoneNumber)
}

pub(super) fn postal_code<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    fan_locale!(locale, rng, address::PostCode)
}

pub(super) fn street_address<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    let building: String = fan_locale!(locale, rng, address::BuildingNumber);
    let street: String = fan_locale!(locale, rng, address::StreetName);
    let city: String = fan_locale!(locale, rng, address::CityName);
    match locale {
        // CJK addresses go big-to-small (prefecture → ward → block →
        // building) and don't concatenate street + building the way
        // Latin-script ones do. This is "less wrong than English
        // ordering," not a faithful rendering.
        Locale::JaJp | Locale::ZhCn | Locale::ZhTw => format!("{city}{street}{building}"),
        _ => format!("{building} {street}, {city}"),
    }
}

pub(super) fn url<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    let user: String = fan_locale!(locale, rng, internet::Username);
    let domain: String = fan_locale!(locale, rng, internet::DomainSuffix);
    let host = sanitise_hostname_label(&user);
    let host = if host.is_empty() { "site" } else { host.as_str() };
    format!("https://www.{host}.{domain}")
}

/// `lat, lon` pair from fake-rs latitude/longitude. They return
/// locale-aware decimal strings already, so this is just a join.
pub(super) fn coordinates<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    let lat: String = fan_locale!(locale, rng, address::Latitude);
    let lon: String = fan_locale!(locale, rng, address::Longitude);
    format!("{lat}, {lon}")
}

/// `fake::automotive::LicencePlate` only ships per-locale impls for
/// a subset (FR/IT/NL/PT-PT/TR/FA). For other locales we synthesise
/// a generic 3-letter + 4-digit shape that looks like an
/// international plate without claiming any specific country.
pub(super) fn license_plate<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    match locale {
        Locale::FrFr => automotive::LicencePlate(FR_FR).fake_with_rng(rng),
        Locale::ItIt => automotive::LicencePlate(IT_IT).fake_with_rng(rng),
        Locale::NlNl => automotive::LicencePlate(NL_NL).fake_with_rng(rng),
        Locale::PtPt | Locale::PtBr => automotive::LicencePlate(PT_PT).fake_with_rng(rng),
        Locale::TrTr => automotive::LicencePlate(TR_TR).fake_with_rng(rng),
        Locale::FaIr => automotive::LicencePlate(FA_IR).fake_with_rng(rng),
        _ => generic_plate(rng),
    }
}

fn generic_plate<R: RngExt + ?Sized>(rng: &mut R) -> String {
    const LETTERS: &[u8; 26] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut letters = String::with_capacity(3);
    for _ in 0..3 {
        let i: usize = (0..LETTERS.len()).fake_with_rng(rng);
        letters.push(LETTERS[i] as char);
    }
    let nums = digits(4, rng);
    format!("{letters}-{nums}")
}

/// Strip characters that aren't valid in a DNS label
/// (RFC 1035: ASCII letters, digits, and hyphens), and trim leading
/// or trailing hyphens. Returns lowercase output.
fn sanitise_hostname_label(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' {
            out.push(ch.to_ascii_lowercase());
        }
    }
    out.trim_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_non_dns_characters() {
        assert_eq!(sanitise_hostname_label("Ali_ce"), "alice");
        assert_eq!(sanitise_hostname_label("Bob.Smith"), "bobsmith");
        assert_eq!(sanitise_hostname_label("-mid-"), "mid");
    }

    #[test]
    fn handles_empty_after_strip() {
        assert!(sanitise_hostname_label("___").is_empty());
    }
}
