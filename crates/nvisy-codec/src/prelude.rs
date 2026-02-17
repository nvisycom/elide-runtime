//! Convenience re-exports.

pub use crate::handler::{
    Handler, Loader, TextEncoding,
    Span, SpanEdit,
    TxtData, TxtHandler, TxtSpan,
    TxtLoader, TxtParams,
    CsvData, CsvHandler, CsvSpan,
    CsvLoader, CsvParams,
    JsonData, JsonHandler, JsonIndent,
    JsonParams, JsonLoader, JsonPath,
};
pub use crate::document::view_stream::SpanStream;
pub use crate::document::edit_stream::SpanEditStream;
pub use crate::document::Document;
