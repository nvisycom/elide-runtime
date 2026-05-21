//! [`DictionaryRegistry`]: named dictionary collection with O(log n) lookup.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::{fmt, fs};

use include_dir::{Dir, include_dir};
use walkdir::WalkDir;

use super::{CsvDictionary, Dictionary, DictionaryLoadError, DictionaryMetadata, TxtDictionary};

const TARGET: &str = "nvisy_pattern::dictionaries";

/// File extension that marks a dictionary sidecar.
const SIDECAR_EXT: &str = "json";

/// A registry of named [`Dictionary`] instances with O(log n) lookup.
///
/// Dictionaries are keyed by name. The name is the slash-normalised
/// relative path under the loaded root, with the file extension
/// stripped — for example `healthcare/drugs.csv` under
/// `assets/dictionaries/` becomes `healthcare/drugs`. The sidecar may
/// override this by setting [`DictionaryMetadata::name`] explicitly.
///
/// Use [`load_builtins`] to populate from compile-time-embedded
/// dictionaries, or [`load_dir`] to walk a filesystem directory
/// recursively at runtime.
///
/// [`load_builtins`]: Self::load_builtins
/// [`load_dir`]: Self::load_dir
#[derive(Default)]
pub struct DictionaryRegistry {
    inner: BTreeMap<String, Box<dyn Dictionary>>,
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
    pub fn insert(&mut self, dict: Box<dyn Dictionary>) {
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

    /// Load all dictionary files from the embedded `assets/dictionaries/`
    /// directory tree into this registry.
    ///
    /// Recurses into subdirectories. The dictionary's default name is
    /// derived from its relative path under the root with the extension
    /// stripped — e.g. `finance/currencies.csv` becomes `finance/currencies`.
    /// A sibling `<stem>.json` sidecar may override this via
    /// [`DictionaryMetadata::name`].
    ///
    /// # Panics
    ///
    /// Panics if any embedded dictionary file is not valid UTF-8,
    /// fails to parse, has an unrecognised extension, or has a
    /// malformed sidecar. Built-in assets are compiled into the
    /// binary, so any of these is a build-time bug that must not be
    /// silently swallowed at runtime.
    #[tracing::instrument(target = TARGET, name = "dictionaries.load_builtins", skip(self), fields(count))]
    pub fn load_builtins(&mut self) {
        static DICT_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/dictionaries");

        for file in walk_embedded(&DICT_DIR) {
            let path = file.path();

            // Sidecars are consumed alongside their dictionary file.
            if extension(path) == Some(SIDECAR_EXT) {
                continue;
            }

            let text = file
                .contents_utf8()
                .expect("built-in dictionary file is not valid UTF-8");

            let default_name = derive_name(path);
            let metadata = load_embedded_metadata(&DICT_DIR, path).unwrap_or_else(|e| {
                panic!(
                    "built-in dictionary '{}' has malformed sidecar: {e}",
                    path.display(),
                )
            });
            let name = metadata.name.clone().unwrap_or(default_name);

            let dict: Box<dyn Dictionary> = match extension(path) {
                Some("txt") => Box::new(TxtDictionary::new(&name, text).with_metadata(metadata)),
                Some("csv") => Box::new(
                    CsvDictionary::new(&name, text)
                        .expect("built-in CSV dictionary must parse")
                        .with_metadata(metadata),
                ),
                other => panic!(
                    "built-in dictionary '{}' has unrecognised extension {other:?}",
                    path.display(),
                ),
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
    /// The dictionary name defaults to the file stem when called
    /// directly. Use [`load_dir`] for path-based naming across an
    /// entire tree.
    ///
    /// If a sibling `<stem>.json` sidecar exists it is parsed for
    /// [`DictionaryMetadata`]; a malformed sidecar logs a warning and
    /// the dictionary is loaded with default metadata. Files with
    /// unrecognised extensions are logged as warnings and ignored.
    ///
    /// `.json` files are silently skipped here so callers can pass
    /// them as part of a directory traversal without producing extra
    /// dictionaries.
    ///
    /// # Errors
    ///
    /// Returns [`nvisy_core::Error`] if the dictionary file itself
    /// cannot be read, or a CSV file fails to parse.
    ///
    /// [`load_dir`]: Self::load_dir
    #[tracing::instrument(target = TARGET, name = "dictionaries.load_file", skip_all, fields(path = %path.as_ref().display()))]
    pub fn load_file(&mut self, path: impl AsRef<Path>) -> nvisy_core::Result<()> {
        let path = path.as_ref();
        let default_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned();
        self.load_file_with_name(path, &default_name)
    }

    /// Load all dictionary files from a filesystem directory tree.
    ///
    /// Recurses into subdirectories. The dictionary's default name is
    /// derived from its path relative to `dir`, with the extension
    /// stripped — `dir/healthcare/drugs.csv` becomes `healthcare/drugs`.
    /// A sidecar's [`name`] field overrides the default verbatim.
    ///
    /// Files with unrecognised extensions are logged as warnings and
    /// skipped. Loaded dictionaries are inserted into `self`, so this
    /// can be called after [`load_builtins`] to layer user-provided
    /// dictionaries on top of the built-ins.
    ///
    /// # Errors
    ///
    /// Returns [`nvisy_core::Error`] if the directory cannot be
    /// traversed, a file cannot be read, or a CSV file fails to parse.
    ///
    /// [`name`]: DictionaryMetadata::name
    /// [`load_builtins`]: Self::load_builtins
    #[tracing::instrument(target = TARGET, name = "dictionaries.load_dir", skip_all, fields(path = %dir.as_ref().display(), count))]
    pub fn load_dir(&mut self, dir: impl AsRef<Path>) -> nvisy_core::Result<()> {
        let dir = dir.as_ref();

        let mut count = 0usize;
        for entry in WalkDir::new(dir).follow_links(false) {
            let entry = entry.map_err(|source| DictionaryLoadError::Walk {
                path: dir.to_owned(),
                source,
            })?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();

            // Sidecars are consumed alongside their dictionary file.
            if extension(path) == Some(SIDECAR_EXT) {
                continue;
            }

            let rel = path.strip_prefix(dir).unwrap_or(path);
            let default_name = derive_name(rel);
            self.load_file_with_name(path, &default_name)?;
            count += 1;
        }

        tracing::Span::current().record("count", count);
        tracing::debug!(target: TARGET, "filesystem dictionaries loaded");
        Ok(())
    }

    fn load_file_with_name(&mut self, path: &Path, default_name: &str) -> nvisy_core::Result<()> {
        // Sidecars themselves: silently skip so a directory traversal
        // doesn't error on .json files.
        if extension(path) == Some(SIDECAR_EXT) {
            return Ok(());
        }

        let metadata = load_sidecar_metadata(path).unwrap_or_else(|e| {
            tracing::warn!(
                target: TARGET,
                path = %path.display(),
                error = %e,
                "dictionary sidecar is malformed, using default metadata",
            );
            DictionaryMetadata::default()
        });
        let name = metadata
            .name
            .clone()
            .unwrap_or_else(|| default_name.to_owned());

        let dict: Box<dyn Dictionary> = match extension(path) {
            Some("txt") => {
                let text =
                    fs::read_to_string(path).map_err(|source| DictionaryLoadError::ReadFile {
                        path: path.to_owned(),
                        source,
                    })?;
                Box::new(TxtDictionary::new(&name, &text).with_metadata(metadata))
            }
            Some("csv") => {
                let text =
                    fs::read_to_string(path).map_err(|source| DictionaryLoadError::ReadFile {
                        path: path.to_owned(),
                        source,
                    })?;
                Box::new(
                    CsvDictionary::new(&name, &text)
                        .map_err(|source| DictionaryLoadError::CsvParse {
                            path: path.to_owned(),
                            source,
                        })?
                        .with_metadata(metadata),
                )
            }
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

/// Recursively iterate every file under an embedded `Dir`.
fn walk_embedded<'a>(dir: &'a Dir<'a>) -> Vec<&'a include_dir::File<'a>> {
    let mut out = Vec::new();
    for f in dir.files() {
        out.push(f);
    }
    for sub in dir.dirs() {
        out.extend(walk_embedded(sub));
    }
    out
}

/// Lowercase ASCII extension lookup.
fn extension(path: &Path) -> Option<&str> {
    path.extension().and_then(|e| e.to_str())
}

/// Convert a relative path into a slash-normalised name with the
/// extension stripped.
fn derive_name(path: &Path) -> String {
    path.with_extension("")
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => s.to_str().map(str::to_owned),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Load `<stem>.json` next to `path` and parse it as
/// [`DictionaryMetadata`]. Returns `Ok(default)` when no sidecar file
/// exists.
fn load_sidecar_metadata(path: &Path) -> Result<DictionaryMetadata, String> {
    let sidecar = path.with_extension(SIDECAR_EXT);
    if !sidecar.exists() {
        return Ok(DictionaryMetadata::default());
    }
    let bytes = fs::read(&sidecar).map_err(|e| format!("read {}: {e}", sidecar.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("parse {}: {e}", sidecar.display()))
}

/// Read the sidecar from the embedded `include_dir` set. Returns
/// `Ok(default)` when no sidecar is embedded for this dictionary.
fn load_embedded_metadata(dir: &Dir<'_>, path: &Path) -> Result<DictionaryMetadata, String> {
    let sidecar_rel: PathBuf = path.with_extension(SIDECAR_EXT);
    let Some(file) = dir.get_file(&sidecar_rel) else {
        return Ok(DictionaryMetadata::default());
    };
    let bytes = file.contents();
    serde_json::from_slice(bytes).map_err(|e| format!("parse {}: {e}", sidecar_rel.display()))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn registry() -> &'static DictionaryRegistry {
        builtin_registry()
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
    fn load_dir_reads_filesystem_flat() {
        let dir = tempfile::tempdir().unwrap();

        fs::write(dir.path().join("colors.txt"), "red\nblue\ngreen\n").unwrap();
        fs::write(dir.path().join("sizes.csv"), "small,S\nmedium,M\nlarge,L\n").unwrap();
        // Should be skipped.
        fs::write(dir.path().join("readme.md"), "ignore me").unwrap();

        let mut reg = DictionaryRegistry::new();
        reg.load_dir(dir.path()).unwrap();

        assert_eq!(reg.len(), 2);
        assert!(reg.get("colors").is_some());
        assert!(reg.get("sizes").is_some());
    }

    #[test]
    fn load_dir_recurses_into_subfolders_with_path_names() {
        let dir = tempfile::tempdir().unwrap();

        fs::create_dir_all(dir.path().join("healthcare")).unwrap();
        fs::create_dir_all(dir.path().join("finance/sub")).unwrap();

        fs::write(
            dir.path().join("healthcare/drugs.txt"),
            "aspirin\nibuprofen\n",
        )
        .unwrap();
        fs::write(dir.path().join("finance/sub/banks.csv"), "Chase\nBoA\n").unwrap();
        fs::write(dir.path().join("top.txt"), "a\nb\n").unwrap();

        let mut reg = DictionaryRegistry::new();
        reg.load_dir(dir.path()).unwrap();

        assert_eq!(reg.len(), 3);
        assert!(reg.get("top").is_some(), "top-level dict keeps short name");
        assert!(
            reg.get("healthcare/drugs").is_some(),
            "subfolder produces path-based name",
        );
        assert!(reg.get("finance/sub/banks").is_some());
    }

    #[test]
    fn sidecar_name_overrides_path() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("finance")).unwrap();
        fs::write(dir.path().join("finance/currencies.csv"), "USD\nEUR\n").unwrap();
        fs::write(
            dir.path().join("finance/currencies.json"),
            r#"{"name": "currencies"}"#,
        )
        .unwrap();

        let mut reg = DictionaryRegistry::new();
        reg.load_dir(dir.path()).unwrap();

        assert!(
            reg.get("currencies").is_some(),
            "sidecar `name` should win over path",
        );
        assert!(
            reg.get("finance/currencies").is_none(),
            "path-based fallback should not also register",
        );
    }

    #[test]
    fn load_dir_missing_directory() {
        let mut reg = DictionaryRegistry::new();
        let result = reg.load_dir("/nonexistent/path");
        assert!(result.is_err());
    }
}
