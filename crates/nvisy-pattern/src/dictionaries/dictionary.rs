//! Core [`Dictionary`] trait, [`DictionaryTerm`], and [`BoxDictionary`] type alias.

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
/// [`TxtDictionary`]: super::TxtDictionary
/// [`CsvDictionary`]: super::CsvDictionary
pub trait Dictionary: Send + Sync {
    /// Unique name identifying this dictionary (e.g. `"nationalities"`).
    fn name(&self) -> &str;

    /// All matchable terms produced by this dictionary.
    fn terms(&self) -> &[DictionaryTerm];
}

/// Type-erased boxed [`Dictionary`].
pub type BoxDictionary = Box<dyn Dictionary>;
