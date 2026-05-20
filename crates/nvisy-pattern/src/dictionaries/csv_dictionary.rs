//! CSV dictionary: one row per entity, each cell becomes a matchable variant.

use super::{CsvDictionaryError, Dictionary, DictionaryMetadata, DictionaryTerm};

/// A dictionary parsed from a CSV file.
///
/// Each row may contain multiple columns (e.g. name, symbol, code).
/// Every non-empty cell becomes a matchable term whose [`column`]
/// records which CSV column it came from.
///
/// [`column`]: DictionaryTerm::column
#[derive(Debug)]
pub struct CsvDictionary {
    name: String,
    terms: Vec<DictionaryTerm>,
    metadata: DictionaryMetadata,
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
    /// Returns `CsvDictionaryError` if any CSV record cannot be parsed.
    pub fn new(name: impl Into<String>, text: &str) -> Result<Self, CsvDictionaryError> {
        let name = name.into();

        let mut terms = Vec::new();
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
                    terms.push(DictionaryTerm {
                        value: field.to_owned(),
                        column: Some(col as u32),
                    });
                }
            }
        }

        Ok(Self {
            name,
            terms,
            metadata: DictionaryMetadata::default(),
        })
    }

    /// Attach metadata loaded from a sidecar file.
    pub fn with_metadata(mut self, metadata: DictionaryMetadata) -> Self {
        self.metadata = metadata;
        self
    }

}

impl Dictionary for CsvDictionary {
    fn name(&self) -> &str {
        &self.name
    }

    fn terms(&self) -> &[DictionaryTerm] {
        &self.terms
    }

    fn metadata(&self) -> &DictionaryMetadata {
        &self.metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rows_with_variants() {
        let dict = CsvDictionary::new("test", "US Dollar,USD\nEuro,EUR\n").unwrap();
        assert_eq!(dict.name(), "test");

        let values: Vec<&str> = dict.terms().iter().map(|t| t.value.as_str()).collect();
        assert_eq!(values, &["US Dollar", "USD", "Euro", "EUR"]);
    }

    #[test]
    fn handles_variable_columns() {
        let dict = CsvDictionary::new("test", "a,b,c\nd,e\n").unwrap();
        let values: Vec<&str> = dict.terms().iter().map(|t| t.value.as_str()).collect();
        assert_eq!(values, &["a", "b", "c", "d", "e"]);
    }

    #[test]
    fn skips_empty_fields() {
        let dict = CsvDictionary::new("test", "a,,b\n").unwrap();
        let values: Vec<&str> = dict.terms().iter().map(|t| t.value.as_str()).collect();
        assert_eq!(values, &["a", "b"]);
    }

    #[test]
    fn column_indices_are_tracked() {
        let dict = CsvDictionary::new("test", "a,b,c\nd,e\n").unwrap();
        let columns: Vec<Option<u32>> = dict.terms().iter().map(|t| t.column).collect();
        assert_eq!(columns, &[Some(0), Some(1), Some(2), Some(0), Some(1)]);
    }
}
