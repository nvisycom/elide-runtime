//! CSV dictionary: one row per entity, each cell is a matchable variant.

use std::path::Path;

use super::Dictionary;

/// Error returned when a CSV dictionary cannot be parsed.
#[derive(Debug, thiserror::Error)]
#[error("failed to parse CSV record in dictionary '{name}': {source}")]
pub struct CsvDictionaryError {
    name: String,
    source: csv::Error,
}

/// A dictionary parsed from a CSV file.
///
/// Each row may contain multiple columns (e.g. name, symbol, code).
/// Every non-empty cell becomes a matchable term.
#[derive(Debug, Clone)]
pub struct CsvDictionary {
    name: String,
    entries: Vec<String>,
    /// Source column index for each entry (parallel to `entries`).
    columns: Vec<usize>,
}

impl CsvDictionary {
    /// Parse a CSV dictionary using the `csv` crate.
    ///
    /// `name` identifies this dictionary (e.g. `"currencies"`).
    /// `text` is the CSV content where each non-empty cell becomes a matchable term.
    /// The column index of each cell is preserved so that per-column confidence
    /// scores can be applied at detection time.
    ///
    /// # Errors
    ///
    /// Returns [`CsvDictionaryError`] if any CSV record cannot be parsed.
    pub fn new(name: impl Into<String>, text: &str) -> Result<Self, CsvDictionaryError> {
        let name = name.into();

        let mut entries = Vec::new();
        let mut columns = Vec::new();
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .trim(csv::Trim::All)
            .from_reader(text.as_bytes());

        for result in reader.records() {
            let record = result.map_err(|source| CsvDictionaryError {
                name: name.clone(),
                source,
            })?;
            for (col, field) in record.iter().enumerate() {
                if !field.is_empty() {
                    entries.push(field.to_owned());
                    columns.push(col);
                }
            }
        }

        Ok(Self {
            name,
            entries,
            columns,
        })
    }

    /// Load a CSV dictionary from a file path.
    ///
    /// The dictionary name is derived from the file stem.
    ///
    /// # Errors
    ///
    /// Returns [`DictionaryLoadError`](super::DictionaryLoadError) if the
    /// file cannot be read or the CSV content cannot be parsed.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, super::DictionaryLoadError> {
        let path = path.as_ref();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let text = std::fs::read_to_string(path).map_err(|source| {
            super::DictionaryLoadError::ReadFile {
                path: path.to_owned(),
                source,
            }
        })?;
        Self::new(name, &text).map_err(|source| super::DictionaryLoadError::CsvParse {
            path: path.to_owned(),
            source,
        })
    }
}

impl Dictionary for CsvDictionary {
    fn name(&self) -> &str {
        &self.name
    }

    fn entries(&self) -> &[String] {
        &self.entries
    }

    fn columns(&self) -> Option<&[usize]> {
        Some(&self.columns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rows_with_variants() {
        let dict = CsvDictionary::new("test", "US Dollar,USD\nEuro,EUR\n").unwrap();
        assert_eq!(dict.name(), "test");
        assert_eq!(dict.entries(), &["US Dollar", "USD", "Euro", "EUR"]);
    }

    #[test]
    fn handles_variable_columns() {
        let dict = CsvDictionary::new("test", "a,b,c\nd,e\n").unwrap();
        assert_eq!(dict.entries(), &["a", "b", "c", "d", "e"]);
    }

    #[test]
    fn skips_empty_fields() {
        let dict = CsvDictionary::new("test", "a,,b\n").unwrap();
        assert_eq!(dict.entries(), &["a", "b"]);
    }

    #[test]
    fn column_indices_are_tracked() {
        let dict = CsvDictionary::new("test", "a,b,c\nd,e\n").unwrap();
        assert_eq!(dict.columns(), Some([0, 1, 2, 0, 1].as_slice()));
    }
}
