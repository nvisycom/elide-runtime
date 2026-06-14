//! Built-in [`Dictionary`]s, embedded at compile time.
//!
//! Each accessor pairs a TOML metadata sidecar
//! (`assets/dictionaries/**/*.toml`) with a term source (`*.csv`
//! for multi-column term lists, `*.txt` for one-per-line), merging
//! them via [`Dictionary::metadata_from_toml`] + [`Term::from_csv`]
//! / [`Term::from_text`].
//!
//! [`Dictionary`]: crate::Dictionary

use crate::recognition::{Dictionary, Term};

macro_rules! shipped_dictionary {
    ($(#[$meta:meta])* fn $name:ident from $meta_path:literal with csv $terms:literal) => {
        $(#[$meta])*
        #[must_use]
        pub fn $name() -> Dictionary {
            let terms = Term::from_csv(include_str!(concat!(
                "../../assets/dictionaries/",
                $terms
            )))
            .expect(concat!("shipped term source `", $terms, "` parses"));
            Dictionary::metadata_from_toml(include_str!(concat!(
                "../../assets/dictionaries/",
                $meta_path
            )))
            .expect(concat!("shipped metadata `", $meta_path, "` is well-formed"))
            .with_terms(terms)
            .build()
            .expect(concat!("shipped dictionary `", $meta_path, "` builds"))
        }
    };
    ($(#[$meta:meta])* fn $name:ident from $meta_path:literal with text $terms:literal) => {
        $(#[$meta])*
        #[must_use]
        pub fn $name() -> Dictionary {
            let terms = Term::from_text(include_str!(concat!(
                "../../assets/dictionaries/",
                $terms
            )));
            Dictionary::metadata_from_toml(include_str!(concat!(
                "../../assets/dictionaries/",
                $meta_path
            )))
            .expect(concat!("shipped metadata `", $meta_path, "` is well-formed"))
            .with_terms(terms)
            .build()
            .expect(concat!("shipped dictionary `", $meta_path, "` builds"))
        }
    };
}

shipped_dictionary!(
    /// Cryptocurrency names and ticker symbols (BTC, Bitcoin, ETH,
    /// Ethereum, …).
    fn cryptocurrencies from "finance/cryptocurrencies.toml" with csv "finance/cryptocurrencies.csv"
);
shipped_dictionary!(
    /// Fiat currency names and ISO 4217 codes (USD, US Dollar, EUR,
    /// Euro, …).
    fn currencies from "finance/currencies.toml" with csv "finance/currencies.csv"
);
shipped_dictionary!(
    /// Human-language names and ISO 639 codes (English, en,
    /// French, fr, …).
    fn languages from "general/languages.toml" with csv "general/languages.csv"
);
shipped_dictionary!(
    /// Demonyms and nationality terms (American, French, …).
    fn nationalities from "general/nationalities.toml" with text "general/nationalities.txt"
);
shipped_dictionary!(
    /// Religious affiliations (Christianity, Islam, …).
    fn religions from "general/religions.toml" with text "general/religions.txt"
);

/// Every built-in dictionary shipped by this crate, in arbitrary
/// stable order.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_dictionary_parses() {
        let dicts = all();
        assert_eq!(dicts.len(), 5);
        for dict in &dicts {
            assert!(!dict.terms.is_empty(), "{} has no terms", dict.name);
        }
    }
}
