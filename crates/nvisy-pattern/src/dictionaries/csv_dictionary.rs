//! CSV dictionary (each row holds variants of a single entity).

use std::path::Path;

use super::Dictionary;

/// A dictionary parsed from a CSV file.
///
/// Each row may contain multiple columns (e.g. name, symbol, code).
/// Every non-empty cell becomes a matchable term.
#[derive(Debug, Clone)]
pub struct CsvDictionary {
    name: String,
    entries: Vec<String>,
}

impl CsvDictionary {
    /// Parse a CSV dictionary using the `csv` crate.
    ///
    /// The dictionary name is derived from the file stem of `path`
    /// (e.g. `"assets/currencies.csv"` → `"currencies"`).
    pub fn new(path: impl AsRef<Path>, text: &str) -> Self {
        let name = path
            .as_ref()
            .file_stem()
            .expect("dictionary path has no file stem")
            .to_string_lossy()
            .into_owned();

        let mut entries = Vec::new();
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .trim(csv::Trim::All)
            .from_reader(text.as_bytes());

        for result in reader.records() {
            let record = result.expect("failed to parse CSV record");
            for field in record.iter() {
                let trimmed = field.trim();
                if !trimmed.is_empty() {
                    entries.push(trimmed.to_owned());
                }
            }
        }

        Self { name, entries }
    }
}

impl Dictionary for CsvDictionary {
    fn name(&self) -> &str {
        &self.name
    }

    fn entries(&self) -> &[String] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rows_with_variants() {
        let dict = CsvDictionary::new("test.csv", "US Dollar,USD\nEuro,EUR\n");
        assert_eq!(dict.name(), "test");
        assert_eq!(dict.entries(), &["US Dollar", "USD", "Euro", "EUR"]);
    }

    #[test]
    fn handles_variable_columns() {
        let dict = CsvDictionary::new("test.csv", "a,b,c\nd,e\n");
        assert_eq!(dict.entries(), &["a", "b", "c", "d", "e"]);
    }

    #[test]
    fn skips_empty_fields() {
        let dict = CsvDictionary::new("test.csv", "a,,b\n");
        assert_eq!(dict.entries(), &["a", "b"]);
    }

    #[test]
    fn derives_name_from_path() {
        let dict = CsvDictionary::new("assets/dictionaries/currencies.csv", "a,b\n");
        assert_eq!(dict.name(), "currencies");
    }
}
