//! Built-in dictionary data for entity matching.
//!
//! Dictionaries are embedded at compile time and loaded lazily on first
//! access.  Two formats are supported:
//!
//! - **Plain text** (`.txt`): one entry per line — see [`TxtDictionary`].
//! - **CSV** (`.csv`): each row holds variants of a single entity
//!   (e.g. `US Dollar,USD`) — see [`CsvDictionary`].

mod csv_dictionary;
mod dictionary;
mod text_dictionary;

pub use csv_dictionary::CsvDictionary;
pub use dictionary::{BoxDictionary, Dictionary};
pub use text_dictionary::TxtDictionary;

use std::collections::BTreeMap;
use std::sync::LazyLock;

use include_dir::{Dir, include_dir};

/// A registry of named dictionaries with O(log n) lookup.
pub struct DictionaryRegistry {
    inner: BTreeMap<String, BoxDictionary>,
}

impl DictionaryRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    /// Insert a dictionary into the registry.
    pub fn insert(&mut self, dict: BoxDictionary) {
        self.inner.insert(dict.name().to_owned(), dict);
    }

    /// Look up a dictionary by name.
    pub fn get(&self, name: &str) -> Option<&dyn Dictionary> {
        self.inner.get(name).map(|b| b.as_ref())
    }

    /// List all dictionary names in sorted order.
    pub fn names(&self) -> Vec<&str> {
        self.inner.keys().map(|s| s.as_str()).collect()
    }

    /// Total number of registered dictionaries.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Load all `.txt` and `.csv` files from the embedded `assets/dictionaries/`
    /// directory and return a populated registry.
    #[tracing::instrument(name = "dictionaries.load_builtins", fields(count))]
    pub fn load_builtins() -> Self {
        static DICT_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/dictionaries");

        let mut reg = Self::new();

        for file in DICT_DIR.files() {
            let path = file.path();
            let text = file
                .contents_utf8()
                .expect("dictionary file is not valid UTF-8");

            let dict: BoxDictionary = match path.extension().and_then(|e| e.to_str()) {
                Some("txt") => Box::new(TxtDictionary::new(path, text)),
                Some("csv") => Box::new(CsvDictionary::new(path, text)),
                other => {
                    tracing::warn!(
                        path = %path.display(),
                        extension = ?other,
                        "skipping unrecognised dictionary file",
                    );
                    continue;
                }
            };

            tracing::trace!(
                name = dict.name(),
                entries = dict.entries().len(),
                "dictionary loaded",
            );
            reg.insert(dict);
        }

        tracing::Span::current().record("count", reg.len());
        tracing::debug!("built-in dictionaries loaded");
        reg
    }
}

impl Default for DictionaryRegistry {
    fn default() -> Self {
        Self::new()
    }
}

static BUILTIN_REGISTRY: LazyLock<DictionaryRegistry> =
    LazyLock::new(DictionaryRegistry::load_builtins);

/// Look up a built-in dictionary by name.
pub fn get_builtin(name: &str) -> Option<&'static [String]> {
    BUILTIN_REGISTRY.get(name).map(|d| d.entries())
}

/// List all available built-in dictionary names.
pub fn list_builtin() -> Vec<&'static str> {
    BUILTIN_REGISTRY.names()
}

/// Get a reference to the built-in [`DictionaryRegistry`].
pub fn builtin_registry() -> &'static DictionaryRegistry {
    &BUILTIN_REGISTRY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_builtin_returns_all_names() {
        let names = list_builtin();
        assert_eq!(names.len(), 5);
        assert!(names.contains(&"cryptocurrencies"));
        assert!(names.contains(&"currencies"));
        assert!(names.contains(&"languages"));
        assert!(names.contains(&"nationalities"));
        assert!(names.contains(&"religions"));
    }

    #[test]
    fn all_listed_builtins_are_loadable() {
        for name in list_builtin() {
            assert!(
                get_builtin(name).is_some(),
                "listed builtin {name} is not loadable"
            );
        }
    }

    #[test]
    fn builtin_dictionaries_are_nonempty() {
        for name in list_builtin() {
            let entries = get_builtin(name).unwrap();
            assert!(
                !entries.is_empty(),
                "builtin dictionary {name} is empty"
            );
        }
    }

    #[test]
    fn nationalities_contains_known_entries() {
        let entries = get_builtin("nationalities").unwrap();
        assert!(entries.iter().any(|e| e == "American"));
        assert!(entries.iter().any(|e| e == "Japanese"));
    }

    #[test]
    fn religions_contains_known_entries() {
        let entries = get_builtin("religions").unwrap();
        assert!(entries.iter().any(|e| e == "Buddhist"));
        assert!(entries.iter().any(|e| e == "Muslim"));
    }

    #[test]
    fn currencies_contains_name_and_code() {
        let entries = get_builtin("currencies").unwrap();
        assert!(entries.iter().any(|e| e == "US Dollar"));
        assert!(entries.iter().any(|e| e == "USD"));
        assert!(entries.iter().any(|e| e == "Euro"));
        assert!(entries.iter().any(|e| e == "EUR"));
    }

    #[test]
    fn cryptocurrencies_contains_name_and_ticker() {
        let entries = get_builtin("cryptocurrencies").unwrap();
        assert!(entries.iter().any(|e| e == "Bitcoin"));
        assert!(entries.iter().any(|e| e == "BTC"));
        assert!(entries.iter().any(|e| e == "Ethereum"));
        assert!(entries.iter().any(|e| e == "ETH"));
    }

    #[test]
    fn languages_contains_name_code_and_aliases() {
        let entries = get_builtin("languages").unwrap();
        assert!(entries.iter().any(|e| e == "English"));
        assert!(entries.iter().any(|e| e == "en"));
        assert!(entries.iter().any(|e| e == "Mandarin"));
        assert!(entries.iter().any(|e| e == "Spanish"));
        assert!(entries.iter().any(|e| e == "Farsi"));
    }

    #[test]
    fn unknown_builtin_returns_none() {
        assert!(get_builtin("nonexistent").is_none());
    }

    #[test]
    fn entries_are_trimmed_and_nonempty() {
        for name in list_builtin() {
            let entries = get_builtin(name).unwrap();
            for entry in entries {
                assert!(!entry.is_empty(), "empty entry in {name}");
                assert_eq!(*entry, entry.trim(), "untrimmed entry in {name}: {entry:?}");
            }
        }
    }

    #[test]
    fn registry_names_are_sorted() {
        let names = list_builtin();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn load_builtins_auto_discovers() {
        let reg = DictionaryRegistry::load_builtins();
        assert_eq!(reg.len(), 5);
    }

    #[test]
    fn registry_insert_and_get() {
        let mut reg = DictionaryRegistry::new();
        reg.insert(Box::new(TxtDictionary::new("test.txt", "foo\nbar\n")));

        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());

        let dict = reg.get("test").unwrap();
        assert_eq!(dict.name(), "test");
        assert_eq!(dict.entries(), &["foo", "bar"]);
    }

    #[test]
    fn registry_unknown_returns_none() {
        let reg = DictionaryRegistry::new();
        assert!(reg.get("nope").is_none());
    }
}
