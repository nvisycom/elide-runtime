//! [`PatternRegistry`]: named pattern collection with O(log n) lookup.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::LazyLock;
use std::{fmt, fs};

use include_dir::{Dir, include_dir};

use super::{BoxPattern, JsonPattern, JsonPatternWarning, Pattern, PatternLoadError};
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
    inner: BTreeMap<String, BoxPattern>,
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
    pub fn insert(&mut self, pattern: BoxPattern) {
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
    pub fn iter(&self) -> impl Iterator<Item = &dyn Pattern> {
        self.inner.values().map(|b| b.as_ref())
    }

    /// Iterate over all registered pattern names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.inner.keys().map(|s| s.as_str())
    }

    /// Total number of registered patterns.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the registry contains no patterns.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Load all `.json` files from the embedded `assets/patterns/`
    /// directory into this registry.
    ///
    /// Files that fail to parse are logged as warnings and skipped.
    #[tracing::instrument(target = TARGET, name = "patterns.load_builtins", skip(self), fields(count))]
    pub fn load_builtins(&mut self) {
        static PATTERN_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/patterns");

        let validators = ValidatorResolver::builtins();

        for file in PATTERN_DIR.files() {
            let path = file.path();

            let Some("json") = path.extension().and_then(|e| e.to_str()) else {
                tracing::warn!(
                    target: TARGET,
                    path = %path.display(),
                    "skipping non-JSON file in patterns directory",
                );
                continue;
            };

            let (pattern, warnings) = match JsonPattern::from_bytes(file.contents(), &validators) {
                Ok(pair) => pair,
                Err(e) => {
                    tracing::warn!(
                        target: TARGET,
                        path = %path.display(),
                        error = %e,
                        "failed to load pattern, skipping",
                    );
                    continue;
                }
            };

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

    /// Load all `.json` files from a filesystem directory.
    ///
    /// Non-`.json` files are logged as warnings and skipped. Loaded
    /// patterns are inserted into `self`, so this can be called after
    /// [`load_builtins`] to layer user-provided
    /// patterns on top of the built-ins.
    ///
    /// # Errors
    ///
    /// Returns [`nvisy_core::Error`] if the directory cannot be read,
    /// a file cannot be read, or a JSON file fails to parse.
    ///
    /// [`load_builtins`]: Self::load_builtins
    #[tracing::instrument(target = TARGET, name = "patterns.load_dir", skip_all, fields(path = %dir.as_ref().display(), count))]
    pub fn load_dir(&mut self, dir: impl AsRef<Path>) -> nvisy_core::Result<()> {
        let dir = dir.as_ref();

        let entries = fs::read_dir(dir).map_err(|source| PatternLoadError::ReadDir {
            path: dir.to_owned(),
            source,
        })?;

        let mut count = 0usize;
        for entry in entries {
            let entry = entry.map_err(|source| PatternLoadError::ReadDir {
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs;

    use super::super::json_pattern::JsonPattern;
    use super::super::pattern::{MatchSource, RegexPattern};
    use super::*;
    use crate::validators::ValidatorResolver;

    fn registry() -> &'static PatternRegistry {
        builtin_registry()
    }

    #[test]
    fn builtins_load() {
        assert!(!registry().is_empty());
    }

    #[test]
    fn pattern_names_are_sorted() {
        let names: Vec<&str> = registry().names().collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn no_duplicate_pattern_names() {
        let names: Vec<_> = registry().names().collect();
        let unique: HashSet<_> = names.iter().collect();
        assert_eq!(names.len(), unique.len(), "duplicate pattern names found");
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
    fn registry_insert_and_get() {
        let validators = ValidatorResolver::builtins();
        let json = br#"{
            "name": "test",
            "category": "personal_identity",
            "entity_type": "government_id",
            "pattern": { "regex": "\\d+", "confidence": 0.9 }
        }"#;
        let (pattern, _warnings) = JsonPattern::from_bytes(json, &validators).unwrap();

        let mut reg = PatternRegistry::new();
        reg.insert(Box::new(pattern));

        assert_eq!(reg.len(), 1);
        assert_eq!(reg.get("test").unwrap().name(), "test");
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
