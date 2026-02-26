//! Core [`Dictionary`] trait and [`BoxDictionary`] alias.

/// A named collection of matchable terms (e.g. nationalities, currencies).
///
/// Implementors load their entries from an asset file at compile time.
/// Two built-in implementations are provided:
///
/// - [`TxtDictionary`]: plain text, one entry per line.
/// - [`CsvDictionary`]: CSV, each cell is a term.
///
/// [`TxtDictionary`]: super::TxtDictionary
/// [`CsvDictionary`]: super::CsvDictionary
pub trait Dictionary: Send + Sync {
    /// Unique name identifying this dictionary (e.g. `"nationalities"`).
    fn name(&self) -> &str;

    /// All matchable terms produced by this dictionary.
    fn entries(&self) -> &[String];

    /// Column index for each entry, parallel to [`entries`](Self::entries).
    ///
    /// Returns `Some` for CSV dictionaries where each cell tracks its
    /// source column. Returns `None` for plain-text dictionaries (all
    /// entries are logically in column 0).
    fn columns(&self) -> Option<&[usize]> {
        None
    }
}

/// Type-erased boxed [`Dictionary`].
pub type BoxDictionary = Box<dyn Dictionary>;
