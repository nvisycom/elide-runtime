//! Universal dictionaries — apply regardless of jurisdiction.

use crate::Dictionary;
use crate::__shipped_dictionary as shipped_dictionary;

shipped_dictionary!(
    /// Cryptocurrency names and ticker symbols (BTC, Bitcoin, ETH,
    /// Ethereum, …).
    fn cryptocurrencies
        from "../../../assets/dictionaries/world/finance/cryptocurrencies.toml"
        with csv "../../../assets/dictionaries/world/finance/cryptocurrencies.csv"
);
shipped_dictionary!(
    /// Fiat currency names and ISO 4217 codes (USD, US Dollar,
    /// EUR, Euro, …).
    fn currencies
        from "../../../assets/dictionaries/world/finance/currencies.toml"
        with csv "../../../assets/dictionaries/world/finance/currencies.csv"
);
shipped_dictionary!(
    /// Human-language names and ISO 639 codes (English, en,
    /// French, fr, …).
    fn languages
        from "../../../assets/dictionaries/world/personal/languages.toml"
        with csv "../../../assets/dictionaries/world/personal/languages.csv"
);
shipped_dictionary!(
    /// Demonyms and nationality terms (American, French, …).
    fn nationalities
        from "../../../assets/dictionaries/world/personal/nationalities.toml"
        with text "../../../assets/dictionaries/world/personal/nationalities.txt"
);
shipped_dictionary!(
    /// Religious affiliations (Christianity, Islam, …).
    fn religions
        from "../../../assets/dictionaries/world/personal/religions.toml"
        with text "../../../assets/dictionaries/world/personal/religions.txt"
);

/// Every world-scoped built-in dictionary.
#[must_use]
pub fn all() -> Vec<Dictionary> {
    vec![
        cryptocurrencies(),
        currencies(),
        languages(),
        nationalities(),
        religions(),
    ]
}
