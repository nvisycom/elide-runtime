//! Financial identifiers and amounts: payment cards, bank
//! accounts, IBAN, SWIFT, currency, monetary values.

use fake::Fake;
use fake::faker::creditcard::raw as creditcard;
use fake::faker::currency::raw as currency;
use fake::faker::finance::raw as finance;
use fake::rand::RngExt;

use super::digits;
use super::dispatch::fan_locale;
use crate::locale::Locale;

pub(super) fn payment_card<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    fan_locale!(locale, rng, creditcard::CreditCardNumber)
}

/// 3-digit CVV. (Amex uses 4 — not worth the locale split for a
/// fake placeholder.)
pub(super) fn card_security_code<R: RngExt + ?Sized>(rng: &mut R) -> String {
    digits(3, rng)
}

/// Locale-aware card expiry. CJK locales render `YYYY年MM月`; the
/// rest use `MM/YY`.
pub(super) fn card_expiry<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    let month: u8 = (1..=12u8).fake_with_rng(rng);
    let year_offset: u8 = (1..=10u8).fake_with_rng(rng);
    let year_full: u16 = 2025 + u16::from(year_offset);
    match locale {
        Locale::JaJp | Locale::ZhCn | Locale::ZhTw => format!("{year_full}年{month}月"),
        _ => {
            let year = (year_full % 100) as u8;
            format!("{month:02}/{year:02}")
        }
    }
}

pub(super) fn currency_code<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    fan_locale!(locale, rng, currency::CurrencyCode)
}

/// Locale-aware monetary amount. SI/UK locales use `1234.56`;
/// continental EU locales use `1234,56`.
pub(super) fn amount<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    let cents: u32 = (0..10_000_000u32).fake_with_rng(rng);
    let whole = cents / 100;
    let frac = cents % 100;
    let sep = decimal_separator(locale);
    format!("{whole}{sep}{frac:02}")
}

/// Plain integer in `0..=10_000`.
pub(super) fn quantity<R: RngExt + ?Sized>(rng: &mut R) -> String {
    let n: u32 = (0..=10_000u32).fake_with_rng(rng);
    n.to_string()
}

pub(super) fn iban<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> Option<String> {
    let (country, total_len) = iban_country(locale)?;
    let body = digits(total_len - 4, rng);
    let check = mod97_check(country, &body);
    Some(format!("{country}{check:02}{body}"))
}

/// Locale-aware bank-account number widths. Rough placeholder, not
/// a faithful mapping of any country's clearing-house standard.
pub(super) fn bank_account<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    digits(bank_account_width(locale), rng)
}

/// 9-digit US ABA routing number. Other countries use bank+branch
/// codes that aren't worth synthesising at this layer.
pub(super) fn bank_routing<R: RngExt + ?Sized>(rng: &mut R) -> String {
    digits(9, rng)
}

/// 8 or 11-character SWIFT/BIC code from fake-rs `finance::Bic`.
pub(super) fn swift_code<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    fan_locale!(locale, rng, finance::Bic)
}

// -- helpers ------------------------------------------------------

fn decimal_separator(locale: Locale) -> char {
    match locale {
        Locale::DeDe | Locale::NlNl | Locale::FrFr | Locale::ItIt | Locale::PtPt | Locale::PtBr => {
            ','
        }
        _ => '.',
    }
}

fn bank_account_width(locale: Locale) -> usize {
    match locale {
        Locale::En => 10,
        Locale::CyGb => 8,
        Locale::FrFr | Locale::ItIt | Locale::PtBr | Locale::PtPt | Locale::NlNl => 11,
        Locale::DeDe => 10,
        Locale::JaJp => 7,
        Locale::ZhCn | Locale::ZhTw => 16,
        Locale::TrTr => 16,
        Locale::ArSa => 12,
        Locale::FaIr => 16,
    }
}

fn iban_country(locale: Locale) -> Option<(&'static str, usize)> {
    match locale {
        Locale::FrFr => Some(("FR", 27)),
        Locale::DeDe => Some(("DE", 22)),
        Locale::ItIt => Some(("IT", 27)),
        Locale::PtPt | Locale::PtBr => Some(("PT", 25)),
        Locale::NlNl => Some(("NL", 18)),
        Locale::TrTr => Some(("TR", 26)),
        Locale::ArSa => Some(("SA", 24)),
        Locale::CyGb => Some(("GB", 22)),
        Locale::En | Locale::JaJp | Locale::ZhCn | Locale::ZhTw | Locale::FaIr => None,
    }
}

/// Compute the two MOD-97 check digits for an IBAN. Letters map to
/// digits via `A=10, B=11, … Z=35`.
fn mod97_check(country: &str, body: &str) -> u8 {
    let mut rearranged = String::with_capacity(body.len() + country.len() + 2);
    rearranged.push_str(body);
    rearranged.push_str(country);
    rearranged.push_str("00");

    let mut remainder: u64 = 0;
    for ch in rearranged.chars() {
        let value: u32 = if ch.is_ascii_digit() {
            ch.to_digit(10).expect("ascii digit")
        } else if ch.is_ascii_alphabetic() {
            (ch.to_ascii_uppercase() as u32 - 'A' as u32) + 10
        } else {
            continue;
        };
        if value >= 10 {
            remainder = (remainder * 100 + u64::from(value)) % 97;
        } else {
            remainder = (remainder * 10 + u64::from(value)) % 97;
        }
    }
    let check = 98u64 - remainder;
    u8::try_from(check).expect("MOD-97 check is in 1..=97")
}

#[cfg(test)]
mod tests {
    use fake::rand::SeedableRng;
    use fake::rand::rngs::SmallRng;

    use super::*;

    fn validate_iban(iban: &str) -> bool {
        let bytes = iban.as_bytes();
        if bytes.len() < 5 {
            return false;
        }
        let (head, tail) = bytes.split_at(4);
        let rearranged: String = tail.iter().chain(head.iter()).map(|b| *b as char).collect();
        let mut remainder: u64 = 0;
        for ch in rearranged.chars() {
            let value: u32 = if ch.is_ascii_digit() {
                ch.to_digit(10).unwrap()
            } else if ch.is_ascii_alphabetic() {
                (ch.to_ascii_uppercase() as u32 - 'A' as u32) + 10
            } else {
                return false;
            };
            if value >= 10 {
                remainder = (remainder * 100 + u64::from(value)) % 97;
            } else {
                remainder = (remainder * 10 + u64::from(value)) % 97;
            }
        }
        remainder == 1
    }

    #[test]
    fn iban_validates_mod97() {
        for locale in [
            Locale::FrFr,
            Locale::DeDe,
            Locale::ItIt,
            Locale::NlNl,
            Locale::TrTr,
            Locale::ArSa,
            Locale::CyGb,
        ] {
            let mut rng = SmallRng::seed_from_u64(99);
            let value = iban(locale, &mut rng).expect("iban locale");
            assert!(validate_iban(&value), "MOD-97 failed for {locale:?}: {value}");
        }
    }

    #[test]
    fn iban_non_locales_return_none() {
        let mut rng = SmallRng::seed_from_u64(99);
        for locale in [
            Locale::En,
            Locale::JaJp,
            Locale::ZhCn,
            Locale::ZhTw,
            Locale::FaIr,
        ] {
            assert!(iban(locale, &mut rng).is_none(), "{locale:?} should be None");
        }
    }

    #[test]
    fn amount_uses_locale_decimal_separator() {
        let mut rng = SmallRng::seed_from_u64(1);
        assert!(amount(Locale::DeDe, &mut rng).contains(','));
        let mut rng = SmallRng::seed_from_u64(1);
        assert!(amount(Locale::En, &mut rng).contains('.'));
    }

    #[test]
    fn card_expiry_uses_cjk_format() {
        let mut rng = SmallRng::seed_from_u64(1);
        assert!(card_expiry(Locale::JaJp, &mut rng).contains('年'));
        let mut rng = SmallRng::seed_from_u64(1);
        assert!(card_expiry(Locale::En, &mut rng).contains('/'));
    }
}
