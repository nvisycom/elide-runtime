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
}

/// Type-erased boxed [`Dictionary`].
pub type BoxDictionary = Box<dyn Dictionary>;
