//! Convenience re-exports for common nvisy-pattern types.

pub use crate::registry::Registry;
pub use crate::dictionaries::{
    BoxDictionary, CsvDictionary, Dictionary, DictionaryRegistry, TxtDictionary,
    builtin_registry as dictionary_registry,
};
pub use crate::patterns::{
    BoxPattern, JsonPattern, Pattern, PatternRegistry,
    builtin_registry as pattern_registry,
};
