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

use std::collections::BTreeMap;
use std::sync::LazyLock;

pub use csv_dictionary::CsvDictionary;
pub use dictionary::{BoxDictionary, Dictionary};
use include_dir::{Dir, include_dir};
pub use text_dictionary::TxtDictionary;

/// A registry of named [`Dictionary`] instances with O(log n) lookup.
///
/// Use [`load_builtins`] to create a registry pre-populated with
/// the compile-time-embedded dictionary files.
///
/// [`load_builtins`]: Self::load_builtins
pub struct DictionaryRegistry {
    inner: BTreeMap<String, BoxDictionary>,
}

impl std::fmt::Debug for DictionaryRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.inner.keys().map(|s| s.as_str()).collect();
        f.debug_struct("DictionaryRegistry")
            .field("len", &self.inner.len())
            .field("names", &names)
            .finish()
    }
}

impl DictionaryRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    /// Insert a dictionary, keyed by its [`Dictionary::name`].
    pub fn insert(&mut self, dict: BoxDictionary) {
        let name = dict.name().to_owned();
        self.inner.insert(name, dict);
    }

    /// Look up a dictionary by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn Dictionary> {
        self.inner.get(name).map(|b| b.as_ref())
    }

    /// Total number of registered dictionaries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
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
    fn builtins_load_and_are_nonempty() {
        let reg = registry();
        assert!(reg.len() > 0);
        for (_, dict) in &reg.inner {
            assert!(!dict.entries().is_empty(), "{} is empty", dict.name());
        }
    }

    #[test]
    fn entries_are_trimmed_and_nonempty() {
        for (_, dict) in &registry().inner {
            let name = dict.name();
            for entry in dict.entries() {
                assert!(!entry.is_empty(), "empty entry in {name}");
                assert_eq!(*entry, entry.trim(), "untrimmed entry in {name}: {entry:?}");
            }
        }
    }

    #[test]
    fn registry_names_are_sorted() {
        let keys: Vec<&str> = registry().inner.keys().map(|s| s.as_str()).collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }

    #[test]
    fn registry_insert_and_get() {
        let mut reg = DictionaryRegistry::new();
        let dict: BoxDictionary = Box::new(TxtDictionary::new("test", "foo\nbar\n"));
        reg.insert(dict);

        assert_eq!(reg.len(), 1);

        let dict = reg.get("test").unwrap();
        assert_eq!(dict.name(), "test");
        assert_eq!(dict.entries(), &["foo", "bar"]);
    }
}
