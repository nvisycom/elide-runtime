//! Street-address composition.

use fake::faker::address::raw as address;
use fake::rand::RngExt;

use super::dispatch::dispatch;
use crate::locale::Locale;

pub(super) fn street_address<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    let building: String = dispatch!(locale, rng, address::BuildingNumber);
    let street: String = dispatch!(locale, rng, address::StreetName);
    let city: String = dispatch!(locale, rng, address::CityName);
    match locale {
        // CJK locales render the city before the street; keep
        // line ordering plausible.
        Locale::JaJp | Locale::ZhCn | Locale::ZhTw => format!("{city}{street}{building}"),
        _ => format!("{building} {street}, {city}"),
    }
}
