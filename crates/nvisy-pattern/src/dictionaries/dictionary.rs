//! Core dictionary trait and type alias.

/// A named set of matchable terms.
pub trait Dictionary: Send + Sync {
    /// Unique name identifying this dictionary (e.g. `"nationalities"`).
    fn name(&self) -> &str;

    /// All matchable terms produced by this dictionary.
    fn entries(&self) -> &[String];
}

/// Type-erased dictionary.
pub type BoxDictionary = Box<dyn Dictionary>;
