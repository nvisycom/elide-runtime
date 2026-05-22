//! Built-in dictionaries for entity matching.
//!
//! Dictionaries are asset files under `assets/dictionaries/` containing
//! matchable terms (nationalities, religions, currencies, etc.). They are
//! embedded at compile time and loaded lazily on first access.
//!
//! Two file formats are supported:
//!
//! - **Plain text** (`.txt`): one entry per line, see [`TxtDictionary`].
//! - **CSV** (`.csv`): each row holds variants of a single entity (e.g.
//!   `US Dollar,USD`), see [`CsvDictionary`].
//!
//! # Key types
//!
//! - [`Dictionary`]: trait implemented by every dictionary.
//! - [`DictionaryRegistry`]: sorted collection with O(log n) lookup.
//!
//! [`TxtDictionary`]: crate::dictionaries::TxtDictionary
//! [`CsvDictionary`]: crate::dictionaries::CsvDictionary
//! [`Dictionary`]: crate::dictionaries::Dictionary
//! [`DictionaryRegistry`]: crate::dictionaries::DictionaryRegistry

mod csv_dictionary;
mod csv_error;
mod dictionary;
mod dictionary_error;
mod dictionary_metadata;
mod dictionary_registry;
mod text_dictionary;

pub(crate) use self::csv_dictionary::CsvDictionary;
pub(crate) use self::csv_error::CsvDictionaryError;
pub(crate) use self::dictionary::{Dictionary, DictionaryCompile, DictionaryTerm};
pub(crate) use self::dictionary_error::DictionaryLoadError;
pub(crate) use self::dictionary_metadata::DictionaryMetadata;
pub(crate) use self::dictionary_registry::{DictionaryRegistry, builtin_registry};
pub(crate) use self::text_dictionary::TxtDictionary;
