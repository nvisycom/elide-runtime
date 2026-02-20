//! Convenience re-exports for common nvisy-pattern types.

pub use crate::dictionaries::{
    BoxDictionary, CsvDictionary, Dictionary, DictionaryRegistry, TxtDictionary,
    builtin_registry as dictionary_registry, get_builtin, list_builtin,
};
pub use crate::patterns::{
    PatternDefinition, PatternRegistry,
    builtin_registry as pattern_registry,
    get_all_pattern_names, get_all_patterns, get_pattern,
};
