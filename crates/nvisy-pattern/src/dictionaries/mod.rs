//! Built-in dictionaries for entity matching.
//!
//! Dictionaries are asset files under `assets/dictionaries/` containing
//! matchable terms (nationalities, religions, currencies, etc.).  They are
//! embedded at compile time and loaded lazily on first access.
//!
//! Two file formats are supported:
//!
//! - **Plain text** (`.txt`): one entry per line, see [`TxtDictionary`].
//! - **CSV** (`.csv`): each row holds variants of a single entity
//!   (e.g. `US Dollar,USD`), see [`CsvDictionary`].
//!
//! # Key types
//!
//! - [`Dictionary`]: trait implemented by every dictionary.
//! - [`DictionaryRegistry`]: sorted collection with O(log n) lookup by name.
//!
//! [`TxtDictionary`]: crate::dictionaries::TxtDictionary
//! [`CsvDictionary`]: crate::dictionaries::CsvDictionary
//! [`Dictionary`]: crate::dictionaries::Dictionary
//! [`DictionaryRegistry`]: crate::dictionaries::DictionaryRegistry

mod csv_dictionary;
mod dictionary;
mod text_dictionary;

pub use csv_dictionary::CsvDictionary;
pub use dictionary::{BoxDictionary, Dictionary};
pub use text_dictionary::TxtDictionary;

use std::sync::LazyLock;

use include_dir::{Dir, include_dir};

use crate::registry::Registry;

/// A registry of named [`Dictionary`] instances with O(log n) lookup.
///
/// Wraps a [`Registry<BoxDictionary>`] and provides convenience accessors
/// that return `&dyn Dictionary` references.
///
/// Use [`load_builtins`] to create a registry pre-populated with
/// the compile-time-embedded dictionary files.
///
/// [`load_builtins`]: Self::load_builtins
pub struct DictionaryRegistry {
    inner: Registry<BoxDictionary>,
}

impl std::fmt::Debug for DictionaryRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DictionaryRegistry")
            .field("len", &self.inner.len())
            .field("names", &self.inner.names())
            .finish()
    }
}

impl DictionaryRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            inner: Registry::new(),
        }
    }

    /// Insert a dictionary, keyed by `name`.
    pub fn insert(&mut self, name: String, dict: BoxDictionary) {
        self.inner.insert(name, dict);
    }

    /// Look up a dictionary by name.
    pub fn get(&self, name: &str) -> Option<&dyn Dictionary> {
        self.inner.get(name).map(|b| b.as_ref())
    }

    /// All dictionary names in deterministic (alphabetical) order.
    pub fn names(&self) -> Vec<&str> {
        self.inner.names()
    }

    /// Total number of registered dictionaries.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Load all `.txt` and `.csv` files from the embedded
    /// `assets/dictionaries/` directory and return a populated registry.
    ///
    /// Unrecognised file extensions are logged as warnings and skipped.
    #[tracing::instrument(name = "dictionaries.load_builtins", fields(count))]
    pub fn load_builtins() -> Self {
        static DICT_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/dictionaries");

        let mut reg = Self::new();

        for file in DICT_DIR.files() {
            let path = file.path();
            let text = file
                .contents_utf8()
                .expect("dictionary file is not valid UTF-8");

            let name = path
                .file_stem()
                .expect("dictionary path has no file stem")
                .to_string_lossy();

            let dict: BoxDictionary = match path.extension().and_then(|e| e.to_str()) {
                Some("txt") => Box::new(TxtDictionary::new(name.as_ref(), text)),
                Some("csv") => Box::new(CsvDictionary::new(name.as_ref(), text)),
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
                %name,
                entries = dict.entries().len(),
                "dictionary loaded",
            );
            reg.insert(name.into_owned(), dict);
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

/// Return a reference to the lazily-initialised built-in [`DictionaryRegistry`].
pub fn builtin_registry() -> &'static DictionaryRegistry {
    &BUILTIN_REGISTRY
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> &'static DictionaryRegistry {
        builtin_registry()
    }

    #[test]
    fn list_builtin_returns_all_names() {
        let names = registry().names();
        assert_eq!(names.len(), 5);
        assert!(names.contains(&"cryptocurrencies"));
        assert!(names.contains(&"currencies"));
        assert!(names.contains(&"languages"));
        assert!(names.contains(&"nationalities"));
        assert!(names.contains(&"religions"));
    }

    #[test]
    fn all_listed_builtins_are_loadable() {
        for name in registry().names() {
            assert!(
                registry().get(name).is_some(),
                "listed builtin {name} is not loadable"
            );
        }
    }

    #[test]
    fn builtin_dictionaries_are_nonempty() {
        for name in registry().names() {
            let dict = registry().get(name).unwrap();
            assert!(
                !dict.entries().is_empty(),
                "builtin dictionary {name} is empty"
            );
        }
    }

    #[test]
    fn nationalities_contains_known_entries() {
        let entries = registry().get("nationalities").unwrap().entries();
        assert!(entries.iter().any(|e| e == "American"));
        assert!(entries.iter().any(|e| e == "Japanese"));
    }

    #[test]
    fn religions_contains_known_entries() {
        let entries = registry().get("religions").unwrap().entries();
        assert!(entries.iter().any(|e| e == "Buddhist"));
        assert!(entries.iter().any(|e| e == "Muslim"));
    }

    #[test]
    fn currencies_contains_name_and_code() {
        let entries = registry().get("currencies").unwrap().entries();
        assert!(entries.iter().any(|e| e == "US Dollar"));
        assert!(entries.iter().any(|e| e == "USD"));
        assert!(entries.iter().any(|e| e == "Euro"));
        assert!(entries.iter().any(|e| e == "EUR"));
    }

    #[test]
    fn cryptocurrencies_contains_name_and_ticker() {
        let entries = registry().get("cryptocurrencies").unwrap().entries();
        assert!(entries.iter().any(|e| e == "Bitcoin"));
        assert!(entries.iter().any(|e| e == "BTC"));
        assert!(entries.iter().any(|e| e == "Ethereum"));
        assert!(entries.iter().any(|e| e == "ETH"));
    }

    #[test]
    fn languages_contains_name_code_and_aliases() {
        let entries = registry().get("languages").unwrap().entries();
        assert!(entries.iter().any(|e| e == "English"));
        assert!(entries.iter().any(|e| e == "en"));
        assert!(entries.iter().any(|e| e == "Mandarin"));
        assert!(entries.iter().any(|e| e == "Spanish"));
        assert!(entries.iter().any(|e| e == "Farsi"));
    }

    #[test]
    fn unknown_builtin_returns_none() {
        assert!(registry().get("nonexistent").is_none());
    }

    #[test]
    fn entries_are_trimmed_and_nonempty() {
        for name in registry().names() {
            let entries = registry().get(name).unwrap().entries();
            for entry in entries {
                assert!(!entry.is_empty(), "empty entry in {name}");
                assert_eq!(*entry, entry.trim(), "untrimmed entry in {name}: {entry:?}");
            }
        }
    }

    #[test]
    fn registry_names_are_sorted() {
        let names = registry().names();
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
        let dict: BoxDictionary = Box::new(TxtDictionary::new("test", "foo\nbar\n"));
        reg.insert("test".into(), dict);

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
