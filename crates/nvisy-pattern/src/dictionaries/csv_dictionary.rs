//! CSV dictionary: one row per entity, each cell is a matchable variant.

use super::Dictionary;

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
    pub fn new(name: impl Into<String>, text: &str) -> Self {
        let name = name.into();

        let mut entries = Vec::new();
        let mut columns = Vec::new();
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .trim(csv::Trim::All)
            .from_reader(text.as_bytes());

        for result in reader.records() {
            let record = result.expect("failed to parse CSV record");
            for (col, field) in record.iter().enumerate() {
                let trimmed = field.trim();
                if !trimmed.is_empty() {
                    entries.push(trimmed.to_owned());
                    columns.push(col);
                }
            }
        }

        Self { name, entries, columns }
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
        let dict = CsvDictionary::new("test", "US Dollar,USD\nEuro,EUR\n");
        assert_eq!(dict.name(), "test");
        assert_eq!(dict.entries(), &["US Dollar", "USD", "Euro", "EUR"]);
    }

    #[test]
    fn handles_variable_columns() {
        let dict = CsvDictionary::new("test", "a,b,c\nd,e\n");
        assert_eq!(dict.entries(), &["a", "b", "c", "d", "e"]);
    }

    #[test]
    fn skips_empty_fields() {
        let dict = CsvDictionary::new("test", "a,,b\n");
        assert_eq!(dict.entries(), &["a", "b"]);
    }
}
