//! Locale-aware bank-account number widths.
//!
//! Lengths approximate common domestic account-number widths, not
//! routing/clearing codes. Output is digits only — locale-specific
//! separators are not added here; format-preserving mode applies any
//! separators the caller wants.

use fake::rand::RngExt;

use super::digits;
use crate::locale::Locale;

pub(super) fn bank_account<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    digits(width(locale), rng)
}

fn width(locale: Locale) -> usize {
    match locale {
        // US ABA account numbers vary 4-17; pick a common 10.
        Locale::En => 10,
        // UK domestic account number is 8 digits.
        Locale::CyGb => 8,
        // SEPA-area domestic numbers cluster around 10-12.
        Locale::FrFr | Locale::ItIt | Locale::PtBr | Locale::PtPt | Locale::NlNl => 11,
        Locale::DeDe => 10,
        Locale::JaJp => 7,
        Locale::ZhCn | Locale::ZhTw => 16,
        Locale::TrTr => 16,
        Locale::ArSa => 12,
        Locale::FaIr => 16,
    }
}

#[cfg(test)]
mod tests {
    use fake::rand::SeedableRng;
    use fake::rand::rngs::SmallRng;

    use super::*;

    #[test]
    fn uk_account_is_eight_digits() {
        let mut rng = SmallRng::seed_from_u64(1);
        let s = bank_account(Locale::CyGb, &mut rng);
        assert_eq!(s.len(), 8);
        assert!(s.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn cn_account_is_sixteen_digits() {
        let mut rng = SmallRng::seed_from_u64(1);
        let s = bank_account(Locale::ZhCn, &mut rng);
        assert_eq!(s.len(), 16);
    }
}
