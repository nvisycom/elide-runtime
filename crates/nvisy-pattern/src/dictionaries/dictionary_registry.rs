//! [`DictionaryRegistry`]: named dictionary collection with O(log n) lookup.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::LazyLock;
use std::{fmt, fs};

use include_dir::{Dir, include_dir};

use super::{BoxDictionary, CsvDictionary, Dictionary, DictionaryLoadError, TxtDictionary};

const TARGET: &str = "nvisy_pattern::dictionaries";

/// A registry of named [`Dictionary`] instances with O(log n) lookup.
///
/// Use [`load_builtins`] to create a registry pre-populated with
/// the compile-time-embedded dictionary files, or [`load_dir`] to
/// load from a filesystem directory at runtime.
///
/// [`load_builtins`]: Self::load_builtins
/// [`load_dir`]: Self::load_dir
#[derive(Default)]
pub struct DictionaryRegistry {
    inner: BTreeMap<String, BoxDictionary>,
}

impl fmt::Debug for DictionaryRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
        Self::default()
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

    /// Iterate over all registered dictionaries as `(name, &dyn Dictionary)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &dyn Dictionary)> {
        self.inner.iter().map(|(k, v)| (k.as_str(), v.as_ref()))
    }

    /// Iterate over all registered dictionary names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.inner.keys().map(|s| s.as_str())
    }

    /// Total number of registered dictionaries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the registry contains no dictionaries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Load all `.txt` and `.csv` files from the embedded
    /// `assets/dictionaries/` directory into this registry.
    ///
    /// Unrecognised file extensions are logged as warnings and skipped.
    #[tracing::instrument(target = TARGET, name = "dictionaries.load_builtins", skip(self), fields(count))]
    pub fn load_builtins(&mut self) {
        static DICT_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/dictionaries");

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
                Some("csv") => Box::new(
                    CsvDictionary::new(name.as_ref(), text)
                        .expect("built-in CSV dictionary must parse"),
                ),
                other => {
                    tracing::warn!(
                        target: TARGET,
                        path = %path.display(),
                        extension = ?other,
                        "skipping unrecognised dictionary file",
                    );
                    continue;
                }
            };

            tracing::trace!(
                target: TARGET,
                name = dict.name(),
                terms = dict.terms().len(),
                "dictionary loaded",
            );
            self.insert(dict);
        }

        tracing::Span::current().record("count", self.len());
        tracing::debug!(target: TARGET, "built-in dictionaries loaded");
    }

    /// Load a single `.txt` or `.csv` dictionary file and insert it.
    ///
    /// The dictionary name is derived from the file stem.
    /// Files with unrecognised extensions are logged as warnings and
    /// ignored (no error is returned).
    ///
    /// # Errors
    ///
    /// Returns [`nvisy_core::Error`] if the file cannot be read or
    /// a CSV file fails to parse.
    #[tracing::instrument(target = TARGET, name = "dictionaries.load_file", skip_all, fields(path = %path.as_ref().display()))]
    pub fn load_file(&mut self, path: impl AsRef<Path>) -> nvisy_core::Result<()> {
        let path = path.as_ref();

        let dict: BoxDictionary = match path.extension().and_then(|e| e.to_str()) {
            Some("txt") => {
                let d = TxtDictionary::from_path(path).map_err(|source| {
                    DictionaryLoadError::ReadFile {
                        path: path.to_owned(),
                        source,
                    }
                })?;
                Box::new(d)
            }
            Some("csv") => Box::new(CsvDictionary::from_path(path)?),
            other => {
                tracing::warn!(
                    target: TARGET,
                    path = %path.display(),
                    extension = ?other,
                    "skipping unrecognised dictionary file",
                );
                return Ok(());
            }
        };

        tracing::trace!(
            target: TARGET,
            name = dict.name(),
            terms = dict.terms().len(),
            "dictionary loaded from filesystem",
        );
        self.insert(dict);
        Ok(())
    }

    /// Load all `.txt` and `.csv` files from a filesystem directory.
    ///
    /// Files with unrecognised extensions are logged as warnings and
    /// skipped. Loaded dictionaries are inserted into `self`, so this
    /// can be called after [`load_builtins`](Self::load_builtins) to
    /// layer user-provided dictionaries on top of the built-ins.
    ///
    /// # Errors
    ///
    /// Returns [`nvisy_core::Error`] if the directory cannot be read,
    /// a file cannot be read, or a CSV file fails to parse.
    #[tracing::instrument(target = TARGET, name = "dictionaries.load_dir", skip_all, fields(path = %dir.as_ref().display(), count))]
    pub fn load_dir(&mut self, dir: impl AsRef<Path>) -> nvisy_core::Result<()> {
        let dir = dir.as_ref();

        let entries = fs::read_dir(dir).map_err(|source| DictionaryLoadError::ReadDir {
            path: dir.to_owned(),
            source,
        })?;

        let mut count = 0usize;
        for entry in entries {
            let entry = entry.map_err(|source| DictionaryLoadError::ReadDir {
                path: dir.to_owned(),
                source,
            })?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            self.load_file(&path)?;
            count += 1;
        }

        tracing::Span::current().record("count", count);
        tracing::debug!(target: TARGET, "filesystem dictionaries loaded");
        Ok(())
    }
}

static BUILTIN_REGISTRY: LazyLock<DictionaryRegistry> = LazyLock::new(|| {
    let mut reg = DictionaryRegistry::new();
    reg.load_builtins();
    reg
});

/// Return a reference to the lazily-initialised built-in [`DictionaryRegistry`].
pub fn builtin_registry() -> &'static DictionaryRegistry {
    &BUILTIN_REGISTRY
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn registry() -> &'static DictionaryRegistry {
        builtin_registry()
    }

    #[test]
    fn builtins_load_and_are_nonempty() {
        let reg = registry();
        assert!(!reg.is_empty());
        for (_, dict) in reg.iter() {
            assert!(!dict.terms().is_empty(), "{} is empty", dict.name());
        }
    }

    #[test]
    fn terms_are_trimmed_and_nonempty() {
        for (_, dict) in registry().iter() {
            let name = dict.name();
            for term in dict.terms() {
                assert!(!term.value.is_empty(), "empty term in {name}");
                assert_eq!(
                    term.value,
                    term.value.trim(),
                    "untrimmed term in {name}: {:?}",
                    term.value,
                );
            }
        }
    }

    #[test]
    fn no_duplicate_terms_per_dictionary() {
        for (_, dict) in registry().iter() {
            let mut seen = HashSet::new();
            for term in dict.terms() {
                assert!(
                    seen.insert(term.value.as_str()),
                    "duplicate term {:?} in dictionary {}",
                    term.value,
                    dict.name(),
                );
            }
        }
    }

    #[test]
    fn registry_names_are_sorted() {
        let keys: Vec<&str> = registry().names().collect();
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

        let values: Vec<&str> = dict.terms().iter().map(|t| t.value.as_str()).collect();
        assert_eq!(values, &["foo", "bar"]);
    }

    #[test]
    fn load_dir_reads_filesystem() {
        let dir = tempfile::tempdir().unwrap();

        fs::write(dir.path().join("colors.txt"), "red\nblue\ngreen\n").unwrap();
        fs::write(dir.path().join("sizes.csv"), "small,S\nmedium,M\nlarge,L\n").unwrap();
        // Should be skipped.
        fs::write(dir.path().join("readme.md"), "ignore me").unwrap();

        let mut reg = DictionaryRegistry::new();
        reg.load_dir(dir.path()).unwrap();

        assert_eq!(reg.len(), 2);

        let colors = reg.get("colors").unwrap();
        let color_values: Vec<&str> = colors.terms().iter().map(|t| t.value.as_str()).collect();
        assert_eq!(color_values, &["red", "blue", "green"]);

        let sizes = reg.get("sizes").unwrap();
        let size_values: Vec<&str> = sizes.terms().iter().map(|t| t.value.as_str()).collect();
        assert_eq!(size_values, &["small", "S", "medium", "M", "large", "L"]);
    }

    #[test]
    fn load_dir_missing_directory() {
        let mut reg = DictionaryRegistry::new();
        let result = reg.load_dir("/nonexistent/path");
        assert!(result.is_err());
    }
}
