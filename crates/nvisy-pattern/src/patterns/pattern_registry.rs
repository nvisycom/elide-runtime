//! [`PatternRegistry`]: named pattern collection with O(log n) lookup.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::LazyLock;
use std::{fmt, fs};

use include_dir::{Dir, include_dir};
use walkdir::WalkDir;

use super::{JsonPattern, JsonPatternWarning, Pattern, PatternLoadError};
use crate::validators::ValidatorResolver;

const TARGET: &str = "nvisy_pattern::patterns";

/// A registry of named [`Pattern`] definitions with O(log n) lookup.
///
/// Use [`load_builtins`] to populate with the compile-time-embedded
/// pattern files, or [`load_dir`] / [`load_file`] to load from the
/// filesystem at runtime.
///
/// [`load_builtins`]: Self::load_builtins
/// [`load_dir`]: Self::load_dir
/// [`load_file`]: Self::load_file
#[derive(Default)]
pub struct PatternRegistry {
    inner: BTreeMap<String, Box<dyn Pattern>>,
}

impl fmt::Debug for PatternRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.inner.keys().map(|s| s.as_str()).collect();
        f.debug_struct("PatternRegistry")
            .field("len", &self.inner.len())
            .field("names", &names)
            .finish()
    }
}

impl PatternRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a pattern, keyed by its [`Pattern::name`].
    pub fn insert(&mut self, pattern: Box<dyn Pattern>) {
        let name = pattern.name().to_owned();
        self.inner.insert(name, pattern);
    }

    /// Look up a pattern by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn Pattern> {
        self.inner.get(name).map(|b| b.as_ref())
    }

    /// Iterate over all registered patterns as `&dyn Pattern` in
    /// deterministic (alphabetical) order.
    ///
    /// Crate-private — the engine builder calls this to walk every
    /// registered pattern when assembling.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &dyn Pattern> {
        self.inner.values().map(|b| b.as_ref())
    }

    /// Total number of registered patterns.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Load all `.json` files from the embedded `assets/patterns/`
    /// directory tree into this registry.
    ///
    /// Recurses into subdirectories. The pattern's name is taken from
    /// the JSON `"name"` field, not the filename.
    ///
    /// # Panics
    ///
    /// Panics if any embedded pattern file fails to parse. Built-in
    /// patterns are compiled into the binary, so a parse failure is a
    /// build-time bug that must not be silently swallowed at runtime.
    #[tracing::instrument(target = TARGET, name = "patterns.load_builtins", skip(self), fields(count))]
    pub fn load_builtins(&mut self) {
        static PATTERN_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/patterns");

        let validators = ValidatorResolver::builtins();

        for file in walk_embedded(&PATTERN_DIR) {
            let path = file.path();

            let Some("json") = path.extension().and_then(|e| e.to_str()) else {
                tracing::warn!(
                    target: TARGET,
                    path = %path.display(),
                    "skipping non-JSON file in patterns directory",
                );
                continue;
            };

            let (pattern, warnings) = JsonPattern::from_bytes(file.contents(), &validators)
                .unwrap_or_else(|e| {
                    panic!("built-in pattern '{}' failed to parse: {e}", path.display(),)
                });

            Self::log_warnings(&warnings);

            tracing::trace!(
                target: TARGET,
                name = %pattern.name(),
                category = %pattern.category(),
                entity_kind = %pattern.entity_kind(),
                match_source = ?pattern.match_source(),
                "pattern loaded",
            );
            self.insert(Box::new(pattern));
        }

        tracing::Span::current().record("count", self.len());
        tracing::debug!(target: TARGET, "built-in patterns loaded");
    }

    /// Load a single `.json` pattern file and insert it.
    ///
    /// The pattern name is derived from the JSON `"name"` field, not
    /// the file name. Files with non-`.json` extensions are logged as
    /// warnings and ignored (no error is returned).
    ///
    /// # Errors
    ///
    /// Returns [`nvisy_core::Error`] if the file cannot be read or
    /// the JSON content cannot be parsed.
    #[tracing::instrument(target = TARGET, name = "patterns.load_file", skip_all, fields(path = %path.as_ref().display()))]
    pub fn load_file(&mut self, path: impl AsRef<Path>) -> nvisy_core::Result<()> {
        let path = path.as_ref();

        let Some("json") = path.extension().and_then(|e| e.to_str()) else {
            tracing::warn!(
                target: TARGET,
                path = %path.display(),
                "skipping non-JSON pattern file",
            );
            return Ok(());
        };

        let bytes = fs::read(path).map_err(|source| PatternLoadError::ReadFile {
            path: path.to_owned(),
            source,
        })?;

        let validators = ValidatorResolver::builtins();
        let (pattern, warnings) =
            JsonPattern::from_bytes(&bytes, &validators).map_err(|source| {
                PatternLoadError::Parse {
                    path: path.to_owned(),
                    source,
                }
            })?;

        Self::log_warnings(&warnings);

        tracing::trace!(
            target: TARGET,
            name = %pattern.name(),
            category = %pattern.category(),
            entity_kind = %pattern.entity_kind(),
            match_source = ?pattern.match_source(),
            "pattern loaded from filesystem",
        );
        self.insert(Box::new(pattern));
        Ok(())
    }

    /// Load all `.json` files from a filesystem directory tree.
    ///
    /// Recurses into subdirectories. Non-`.json` files are logged as
    /// warnings and skipped. Loaded patterns are inserted into `self`,
    /// so this can be called after [`load_builtins`] to layer
    /// user-provided patterns on top of the built-ins.
    ///
    /// # Errors
    ///
    /// Returns [`nvisy_core::Error`] if the directory cannot be
    /// traversed, a file cannot be read, or a JSON file fails to parse.
    ///
    /// [`load_builtins`]: Self::load_builtins
    #[tracing::instrument(target = TARGET, name = "patterns.load_dir", skip_all, fields(path = %dir.as_ref().display(), count))]
    pub fn load_dir(&mut self, dir: impl AsRef<Path>) -> nvisy_core::Result<()> {
        let dir = dir.as_ref();

        let mut count = 0usize;
        for entry in WalkDir::new(dir).follow_links(false) {
            let entry = entry.map_err(|source| PatternLoadError::Walk {
                path: dir.to_owned(),
                source,
            })?;
            if !entry.file_type().is_file() {
                continue;
            }
            self.load_file(entry.path())?;
            count += 1;
        }

        tracing::Span::current().record("count", count);
        tracing::debug!(target: TARGET, "filesystem patterns loaded");
        Ok(())
    }

    fn log_warnings(warnings: &[JsonPatternWarning]) {
        for w in warnings {
            match w {
                JsonPatternWarning::UnknownValidator { pattern, validator } => {
                    tracing::warn!(
                        target: TARGET,
                        %pattern,
                        %validator,
                        "unknown validator name, pattern will have no post-match validation",
                    );
                }
            }
        }
    }
}

static BUILTIN_REGISTRY: LazyLock<PatternRegistry> = LazyLock::new(|| {
    let mut reg = PatternRegistry::new();
    reg.load_builtins();
    reg
});

/// Return a reference to the lazily-initialised built-in [`PatternRegistry`].
pub fn builtin_registry() -> &'static PatternRegistry {
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::pattern::{MatchSource, RegexPattern};
    use super::*;
    use crate::validators::ValidatorResolver;

    fn registry() -> &'static PatternRegistry {
        builtin_registry()
    }

    #[test]
    fn all_patterns_have_valid_fields() {
        for p in registry().iter() {
            assert!(!p.name().is_empty(), "pattern name is empty");
            match p.match_source() {
                MatchSource::Regex(rp) => {
                    assert!(!rp.regex.is_empty(), "regex is empty for {}", p.name());
                    assert!(rp.confidence > 0.0, "confidence is 0 for {}", p.name());
                    assert!(rp.confidence <= 1.0, "confidence > 1 for {}", p.name());
                }
                MatchSource::Dictionary(dp) => {
                    assert!(!dp.name.is_empty(), "dictionary is empty for {}", p.name());
                    let c = dp.confidence.resolve(0);
                    assert!(c > 0.0, "confidence is 0 for {}", p.name());
                    assert!(c <= 1.0, "confidence > 1 for {}", p.name());
                }
            }
        }
    }

    #[test]
    fn all_regex_patterns_compile() {
        for p in registry().iter() {
            if let MatchSource::Regex(rp) = p.match_source() {
                assert!(
                    regex::Regex::new(&rp.effective_regex()).is_ok(),
                    "pattern {} failed to compile: {}",
                    p.name(),
                    rp.regex,
                );
            }
        }
    }

    #[test]
    fn all_validators_resolve() {
        let resolver = ValidatorResolver::builtins();
        for p in registry().iter() {
            if let MatchSource::Regex(RegexPattern {
                validator: Some(name),
                ..
            }) = p.match_source()
            {
                assert!(
                    resolver.resolve(name).is_some(),
                    "pattern {} references unregistered validator {name}",
                    p.name(),
                );
            }
        }
    }

    #[test]
    fn load_dir_reads_filesystem() {
        let dir = tempfile::tempdir().unwrap();

        fs::write(
            dir.path().join("test_pattern.json"),
            r#"{
                "name": "test_fs",
                "category": "personal_identity",
                "entity_type": "government_id",
                "pattern": { "regex": "\\d{3}", "confidence": 0.8 }
            }"#,
        )
        .unwrap();
        // Should be skipped.
        fs::write(dir.path().join("readme.md"), "ignore me").unwrap();

        let mut reg = PatternRegistry::new();
        reg.load_dir(dir.path()).unwrap();

        assert_eq!(reg.len(), 1);
        assert_eq!(reg.get("test_fs").unwrap().name(), "test_fs");
    }

    #[test]
    fn load_dir_recurses_into_subfolders() {
        let dir = tempfile::tempdir().unwrap();

        fs::create_dir_all(dir.path().join("contact")).unwrap();
        fs::write(
            dir.path().join("contact/email_nested.json"),
            r#"{
                "name": "email_nested",
                "category": "contact_info",
                "entity_type": "email_address",
                "pattern": { "regex": ".+@.+", "confidence": 0.7 }
            }"#,
        )
        .unwrap();
        fs::write(
            dir.path().join("top.json"),
            r#"{
                "name": "top",
                "category": "personal_identity",
                "entity_type": "government_id",
                "pattern": { "regex": "\\d{3}", "confidence": 0.8 }
            }"#,
        )
        .unwrap();

        let mut reg = PatternRegistry::new();
        reg.load_dir(dir.path()).unwrap();

        assert_eq!(reg.len(), 2);
        assert!(reg.get("top").is_some());
        assert!(
            reg.get("email_nested").is_some(),
            "pattern in subfolder should be loaded"
        );
    }

    #[test]
    fn load_dir_missing_directory() {
        let mut reg = PatternRegistry::new();
        let result = reg.load_dir("/nonexistent/path");
        assert!(result.is_err());
    }

    #[test]
    fn load_file_single_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("single.json");
        fs::write(
            &path,
            r#"{
                "name": "single_test",
                "category": "contact_info",
                "entity_type": "email_address",
                "pattern": { "regex": ".+@.+", "confidence": 0.7 }
            }"#,
        )
        .unwrap();

        let mut reg = PatternRegistry::new();
        reg.load_file(&path).unwrap();

        assert_eq!(reg.len(), 1);
        assert_eq!(reg.get("single_test").unwrap().name(), "single_test");
    }
}
