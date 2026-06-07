//! Street-address composition.

use fake::faker::address::raw as address;
use fake::rand::RngExt;

use super::dispatch::fan_locale;
use crate::locale::Locale;

pub(super) fn street_address<R: RngExt + ?Sized>(locale: Locale, rng: &mut R) -> String {
    let building: String = fan_locale!(locale, rng, address::BuildingNumber);
    let street: String = fan_locale!(locale, rng, address::StreetName);
    let city: String = fan_locale!(locale, rng, address::CityName);
    match locale {
        // CJK addresses go big-to-small (prefecture → ward → block →
        // building) and don't actually concatenate street + building
        // the way Latin-script ones do. This is "less wrong than
        // English ordering," not a faithful rendering — good enough
        // for a fake placeholder, not a real localisation.
        Locale::JaJp | Locale::ZhCn | Locale::ZhTw => format!("{city}{street}{building}"),
        _ => format!("{building} {street}, {city}"),
    }
}
