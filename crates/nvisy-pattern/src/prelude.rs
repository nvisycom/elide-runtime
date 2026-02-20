//! Convenience re-exports for common nvisy-pattern types.

pub use crate::registry::Registry;
pub use crate::dictionaries::{
    BoxDictionary, CsvDictionary, Dictionary, DictionaryRegistry, TxtDictionary,
    builtin_registry as dictionary_registry,
};
pub use crate::patterns::{
    BoxPattern, JsonPattern, MatchSource, Pattern, PatternRegistry,
    builtin_registry as pattern_registry,
};
pub use crate::validators::{ValidatorFn, ValidatorResolver};
