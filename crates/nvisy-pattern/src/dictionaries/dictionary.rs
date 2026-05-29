//! Core [`Dictionary`] trait and [`DictionaryTerm`].

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, BuildError};

use super::DictionaryMetadata;

/// A single matchable term within a [`Dictionary`].
///
/// Each term carries its matched value and, for multi-column sources like
/// CSV files, the column index it originated from. Plain-text dictionaries
/// leave `column` as `None` (logically equivalent to column 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryTerm {
    /// The matchable text value.
    pub value: String,
    /// Source column index for CSV dictionaries.
    ///
    /// `None` for plain-text dictionaries where column position is
    /// not meaningful.
    pub column: Option<u32>,
}

/// A named collection of matchable terms (e.g. nationalities, currencies).
///
/// Two built-in implementations are provided:
///
/// - [`TxtDictionary`]: plain text, one entry per line.
/// - [`CsvDictionary`]: CSV, each cell is a term with its column index.
///
/// This trait is **sealed**: external crates cannot add new implementations.
/// New dictionary sources should be added via files loaded through
/// [`DictionaryRegistry::load_dir`] or [`DictionaryRegistry::load_file`].
///
/// [`TxtDictionary`]: super::TxtDictionary
/// [`CsvDictionary`]: super::CsvDictionary
/// [`DictionaryRegistry::load_dir`]: super::DictionaryRegistry::load_dir
/// [`DictionaryRegistry::load_file`]: super::DictionaryRegistry::load_file
pub trait Dictionary: sealed::Sealed + Send + Sync {
    /// Unique name identifying this dictionary (e.g. `"nationalities"`).
    fn name(&self) -> &str;

    /// All matchable terms produced by this dictionary.
    fn terms(&self) -> &[DictionaryTerm];

    /// Optional metadata loaded from this dictionary's `name.json`
    /// sidecar (language/industry/region tags, version, description).
    ///
    /// Returns a reference into the dictionary's own storage; the
    /// default returns an empty metadata value, used when no sidecar
    /// was loaded.
    fn metadata(&self) -> &DictionaryMetadata {
        static EMPTY: std::sync::LazyLock<DictionaryMetadata> =
            std::sync::LazyLock::new(DictionaryMetadata::default);
        &EMPTY
    }
}

/// Engine-internal extension that compiles a [`Dictionary`] into an
/// Aho-Corasick automaton.
///
/// Lives behind a crate-private trait so the public [`Dictionary`]
/// surface stays unaware of the engine-internal `aho_corasick`
/// types. Has a blanket impl over every `Dictionary`; consumers
/// only call this from inside the engine builder.
pub(crate) trait DictionaryCompile {
    /// Compile this dictionary's terms into an Aho-Corasick automaton.
    ///
    /// `case_sensitive` (sourced per-use from the
    /// `DictionaryPattern` that references this dictionary) controls
    /// the automaton's `ascii_case_insensitive` setting (inverted:
    /// `false` here produces a case-insensitive automaton). Term
    /// order in the built automaton matches [`terms`]'s order, so
    /// callers can resolve pattern ids by index.
    ///
    /// [`terms`]: Dictionary::terms
    fn build_automaton(&self, case_sensitive: bool) -> Result<AhoCorasick, BuildError>;
}

impl<D: Dictionary + ?Sized> DictionaryCompile for D {
    fn build_automaton(&self, case_sensitive: bool) -> Result<AhoCorasick, BuildError> {
        AhoCorasickBuilder::new()
            .ascii_case_insensitive(!case_sensitive)
            .build(self.terms().iter().map(|t| t.value.as_str()))
    }
}

pub(crate) mod sealed {
    use super::super::csv_dictionary::CsvDictionary;
    use super::super::text_dictionary::TxtDictionary;

    pub trait Sealed {}
    impl Sealed for CsvDictionary {}
    impl Sealed for TxtDictionary {}
}
