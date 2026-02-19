//! Convenience re-exports.

pub use crate::handler::{
    Handler, Loader,
    Span, SpanEdit,
    TxtHandler, TxtSpan,
    TxtLoader, TxtParams,
    CsvData, CsvHandler, CsvSpan,
    CsvLoader, CsvParams,
    JsonData, JsonHandler, JsonIndent,
    JsonParams, JsonLoader, JsonPath,
    JpegHandler, JpegLoader, JpegParams,
    PngHandler, PngLoader, PngParams,
};
#[cfg(feature = "html")]
pub use crate::handler::{HtmlData, HtmlHandler, HtmlSpan, HtmlLoader, HtmlParams};
pub use crate::document::{Document, SpanEditStream, SpanStream};
pub use crate::transform::{
    AudioHandler, AudioRedaction, AudioRedactionOutput,
    ImageHandler, ImageRedaction, ImageRedactionOutput, ImageTransform,
    TextHandler, TextRedaction, TextRedactionOutput,
};
