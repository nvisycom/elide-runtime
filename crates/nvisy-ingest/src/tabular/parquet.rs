//! Apache Parquet file loader.

use serde::Deserialize;
use std::sync::Arc;

use nvisy_core::io::ContentData;
use nvisy_core::error::{Error, ErrorKind};

use crate::document::Document;
use crate::handler::{ParquetHandler, PlaintextHandler, FormatHandler, SpreadsheetLoader};

use arrow::array::{Array, RecordBatchReader};
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

/// Typed parameters for [`ParquetLoader`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParquetLoaderParams {
    /// Maximum number of rows to read. `None` means all rows.
    #[serde(default)]
    pub max_rows: Option<usize>,
}

/// Extracts tabular data from Parquet files plus a flattened text document
/// for regex/dictionary scanning.
pub struct ParquetLoader;

impl Clone for ParquetLoader {
    fn clone(&self) -> Self { Self }
}

#[async_trait::async_trait]
impl SpreadsheetLoader for ParquetLoader {
    type Params = ParquetLoaderParams;

    async fn load(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<Vec<Document<FormatHandler>>, Error> {
        let data = bytes::Bytes::copy_from_slice(&content.to_bytes());
        let builder = ParquetRecordBatchReaderBuilder::try_new(data)
            .map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("Parquet open failed: {e}"))
            })?;

        let reader = builder.build().map_err(|e| {
            Error::new(ErrorKind::Runtime, format!("Parquet reader build failed: {e}"))
        })?;

        let schema = reader.schema();
        let columns: Vec<String> = schema
            .fields()
            .iter()
            .map(|f: &arrow::datatypes::FieldRef| f.name().clone())
            .collect();

        let mut all_rows: Vec<Vec<String>> = Vec::new();

        for batch_result in reader {
            let batch: RecordBatch = batch_result.map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("Parquet batch read failed: {e}"))
            })?;

            for row_idx in 0..batch.num_rows() {
                if let Some(max) = params.max_rows {
                    if all_rows.len() >= max {
                        break;
                    }
                }

                let mut row = Vec::with_capacity(batch.num_columns());
                for col_idx in 0..batch.num_columns() {
                    let col: &Arc<dyn Array> = batch.column(col_idx);
                    let val = array_value_to_string(col.as_ref(), row_idx);
                    row.push(val);
                }
                all_rows.push(row);
            }

            if let Some(max) = params.max_rows {
                if all_rows.len() >= max {
                    break;
                }
            }
        }

        let mut tabular_doc = Document::new(ParquetHandler)
            .with_tabular(columns, all_rows.clone());
        tabular_doc.source.set_parent_id(Some(content.content_source.as_uuid()));

        // Flatten to text for regex/dictionary scanning
        let mut text_parts = Vec::new();
        for row in &all_rows {
            text_parts.push(row.join("\t"));
        }
        let flat_text = text_parts.join("\n");
        let mut text_doc = Document::new(PlaintextHandler)
            .with_text(flat_text);
        text_doc.source.set_parent_id(Some(content.content_source.as_uuid()));

        Ok(vec![tabular_doc.into_format(), text_doc.into_format()])
    }
}

fn array_value_to_string(array: &dyn Array, index: usize) -> String {
    if array.is_null(index) {
        return String::new();
    }

    use std::fmt::Write;
    let mut buf = String::new();
    let formatter = arrow::util::display::ArrayFormatter::try_new(array, &Default::default());
    match formatter {
        Ok(f) => {
            let _ = write!(buf, "{}", f.value(index));
            buf
        }
        Err(_) => String::new(),
    }
}

impl crate::handler::Handler for ParquetLoader {
    fn id(&self) -> &str { ParquetHandler.id() }
    fn extensions(&self) -> &[&str] { ParquetHandler.extensions() }
    fn content_types(&self) -> &[&str] { ParquetHandler.content_types() }
}
