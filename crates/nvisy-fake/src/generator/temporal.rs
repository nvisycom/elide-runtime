//! Locale-aware date / date-time / age rendering.
//!
//! Both `DateOfBirth` and `DateTime` go through this module — the
//! only differences are the year range (DOB picks an adult-ish
//! window; DateTime hovers near "now") and whether a time
//! component is appended.

use fake::Fake;
use fake::rand::RngExt;

use crate::locale::Locale;

pub(super) fn age<R: RngExt + ?Sized>(rng: &mut R) -> String {
    let years: u8 = (1..=99u8).fake_with_rng(rng);
    years.to_string()
}

/// Year in `[1940, 2010]`, month in `[1, 12]`, day in `[1, 28]` so
/// every month stays valid without a date library.
pub(super) fn date_of_birth<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    let year: u16 = (1940..=2010u16).fake_with_rng(rng);
    let month: u8 = (1..=12u8).fake_with_rng(rng);
    let day: u8 = (1..=28u8).fake_with_rng(rng);
    format_date(locale, year, month, day)
}

/// Date-time near 2020-2030. Locale-aware date format plus
/// `HH:MM:SS` (or `HH時MM分SS秒` in CJK locales).
pub(super) fn date_time<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    let year: u16 = (2020..=2030u16).fake_with_rng(rng);
    let month: u8 = (1..=12u8).fake_with_rng(rng);
    let day: u8 = (1..=28u8).fake_with_rng(rng);
    let hour: u8 = (0..=23u8).fake_with_rng(rng);
    let minute: u8 = (0..=59u8).fake_with_rng(rng);
    let second: u8 = (0..=59u8).fake_with_rng(rng);
    let date = format_date(locale, year, month, day);
    let time = format_time(locale, hour, minute, second);
    format!("{date} {time}")
}

fn format_date(locale: Locale, year: u16, month: u8, day: u8) -> String {
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

fn format_time(locale: Locale, hour: u8, minute: u8, second: u8) -> String {
    match locale {
        Locale::JaJp | Locale::ZhCn | Locale::ZhTw => {
            format!("{hour}時{minute}分{second}秒")
        }
        _ => format!("{hour:02}:{minute:02}:{second:02}"),
    }
}

#[cfg(test)]
mod tests {
    use fake::rand::SeedableRng;
    use fake::rand::rngs::SmallRng;

    use super::*;

    #[test]
    fn dob_iso_default() {
        let mut rng = SmallRng::seed_from_u64(1);
        let s = date_of_birth(Locale::En, &mut rng);
        assert_eq!(s.len(), 10);
        assert_eq!(&s[4..5], "-");
    }

    #[test]
    fn dob_japanese_format_uses_kanji() {
        let mut rng = SmallRng::seed_from_u64(1);
        let s = date_of_birth(Locale::JaJp, &mut rng);
        assert!(s.contains('年') && s.contains('月') && s.contains('日'));
    }

    #[test]
    fn dob_german_format_uses_dots() {
        let mut rng = SmallRng::seed_from_u64(1);
        let s = date_of_birth(Locale::DeDe, &mut rng);
        assert_eq!(s.matches('.').count(), 2);
    }

    #[test]
    fn datetime_includes_time_component() {
        let mut rng = SmallRng::seed_from_u64(1);
        let s = date_time(Locale::En, &mut rng);
        assert!(s.contains(' ') && s.matches(':').count() == 2);
    }

    #[test]
    fn datetime_japanese_uses_kanji_for_time() {
        let mut rng = SmallRng::seed_from_u64(1);
        let s = date_time(Locale::JaJp, &mut rng);
        assert!(s.contains('時') && s.contains('分') && s.contains('秒'));
    }
}
