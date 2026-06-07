//! Locale-aware date-of-birth rendering.

use fake::Fake;
use fake::rand::RngExt;

use crate::locale::Locale;

/// Synthesise a date of birth without pulling in `chrono`: year in
/// `[1940, 2010]`, month in `[1, 12]`, day in `[1, 28]` so every
/// month stays valid.
pub(super) fn date_of_birth<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    let year: u16 = (1940..=2010u16).fake_with_rng(rng);
    let month: u8 = (1..=12u8).fake_with_rng(rng);
    let day: u8 = (1..=28u8).fake_with_rng(rng);
    match locale {
        Locale::JaJp | Locale::ZhCn | Locale::ZhTw => {
            format!("{year:04}年{month}月{day}日")
        }
        Locale::DeDe | Locale::NlNl => format!("{day:02}.{month:02}.{year:04}"),
        Locale::FrFr | Locale::ItIt | Locale::PtPt | Locale::PtBr => {
            format!("{day:02}/{month:02}/{year:04}")
        }
        _ => format!("{year:04}-{month:02}-{day:02}"),
    }
}

#[cfg(test)]
mod tests {
    use fake::rand::SeedableRng;
    use fake::rand::rngs::SmallRng;

    use super::*;

    #[test]
    fn iso_default() {
        let mut rng = SmallRng::seed_from_u64(1);
        let s = date_of_birth(Locale::En, &mut rng);
        assert_eq!(s.len(), 10);
        assert_eq!(&s[4..5], "-");
    }

    #[test]
    fn japanese_format_uses_kanji() {
        let mut rng = SmallRng::seed_from_u64(1);
        let s = date_of_birth(Locale::JaJp, &mut rng);
        assert!(s.contains('年') && s.contains('月') && s.contains('日'));
    }

    #[test]
    fn german_format_uses_dots() {
        let mut rng = SmallRng::seed_from_u64(1);
        let s = date_of_birth(Locale::DeDe, &mut rng);
        assert_eq!(s.matches('.').count(), 2);
    }
}
