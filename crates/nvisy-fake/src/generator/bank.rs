//! Locale-aware bank-account number widths.
//!
//! Output is digits only — locale-specific separators are not
//! applied here; format-preserving mode adds them from the original
//! span when requested.
//!
//! The width table below is a rough placeholder, not a faithful
//! mapping of any country's clearing-house standard. Account-number
//! widths in the real world vary by bank within each country (US
//! ABA accounts are 4-17 digits; SEPA domestic numbers cluster
//! 10-12 but have outliers; etc.). The values are picked to look
//! plausible for the locale, not to validate against any system.

use fake::rand::RngExt;

use super::digits;
use crate::locale::Locale;

pub(super) fn bank_account<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    digits(width(locale), rng)
}

fn width(locale: Locale) -> usize {
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

#[cfg(test)]
mod tests {
    use fake::rand::SeedableRng;
    use fake::rand::rngs::SmallRng;

    use super::*;

    #[test]
    fn uk_account_matches_table_width() {
        let mut rng = SmallRng::seed_from_u64(1);
        let s = bank_account(Locale::CyGb, &mut rng);
        assert_eq!(s.len(), 8);
        assert!(s.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn cn_account_matches_table_width() {
        let mut rng = SmallRng::seed_from_u64(1);
        let s = bank_account(Locale::ZhCn, &mut rng);
        assert_eq!(s.len(), 16);
    }
}
