//! IBAN synthesis with MOD-97 check digits.
//!
//! Picks a real country prefix per [`Locale`], generates an account
//! body of the country's standard length, and computes the check
//! digits per the ISO 13616 / MOD-97-10 scheme so the resulting IBAN
//! validates against any standard checker.
//!
//! Returns `None` for locales whose country has no national IBAN
//! scheme (United States, Japan, China, Iran) — the caller falls
//! through to the `[{entity_kind}]` placeholder rather than minting
//! a misleading foreign IBAN.

use fake::rand::RngExt;

use super::digits;
use crate::locale::Locale;

pub(super) fn iban<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> Option<String> {
    let (country, total_len) = country_for(locale)?;
    let body = digits(total_len - 4, rng);
    let check = mod97_check(country, &body);
    Some(format!("{country}{check:02}{body}"))
}

/// Map a locale to a representative IBAN country code and total
/// length, or `None` for countries without a national IBAN scheme.
/// Lengths sourced from the ISO 13616 registry.
fn country_for(locale: Locale) -> Option<(&'static str, usize)> {
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

/// Compute the two MOD-97 check digits for an IBAN whose country
/// prefix is `country` and whose account body (the part after the
/// check digits) is `body`. Letters map to digits via `A=10, B=11,
/// … Z=35`.
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

    /// Recomputes the check digit on a candidate IBAN: an IBAN is
    /// valid iff `mod97(rearranged) == 1`.
    fn validate(iban: &str) -> bool {
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
            assert!(validate(&value), "MOD-97 failed for {locale:?}: {value}");
        }
    }

    #[test]
    fn iban_total_length_matches_country() {
        let mut rng = SmallRng::seed_from_u64(99);
        assert_eq!(iban(Locale::FrFr, &mut rng).expect("FR").len(), 27);
        assert_eq!(iban(Locale::DeDe, &mut rng).expect("DE").len(), 22);
        assert_eq!(iban(Locale::NlNl, &mut rng).expect("NL").len(), 18);
    }

    #[test]
    fn non_iban_locales_return_none() {
        let mut rng = SmallRng::seed_from_u64(99);
        for locale in [
            Locale::En,
            Locale::JaJp,
            Locale::ZhCn,
            Locale::ZhTw,
            Locale::FaIr,
        ] {
            assert!(
                iban(locale, &mut rng).is_none(),
                "{locale:?} should be None"
            );
        }
    }
}
