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
    ImageData, AnyImage,
    JpegHandler, JpegLoader, JpegParams,
    PngHandler, PngLoader, PngParams,
    AnyAudio,
    WavHandler, WavLoader, WavParams,
    Mp3Handler, Mp3Loader, Mp3Params,
};
#[cfg(feature = "html")]
pub use crate::handler::{HtmlData, HtmlHandler, HtmlSpan, HtmlLoader, HtmlParams};
#[cfg(feature = "pdf")]
pub use crate::handler::{PdfHandler, PdfSpan, PdfLoader, PdfParams};
#[cfg(feature = "docx")]
pub use crate::handler::{DocxHandler, DocxLoader, DocxParams};
#[cfg(feature = "xlsx")]
pub use crate::handler::{XlsxHandler, XlsxLoader, XlsxParams};
pub use crate::document::{AnyDocument, UniversalLoader, Document};
pub use crate::stream::{SpanEditStream, SpanStream};
pub use crate::transform::{
    AudioHandler, AudioRedaction, AudioRedactionOutput,
    ImageHandler, ImageRedaction, ImageRedactionOutput, ImageTransform,
    TextHandler, TextRedaction, TextRedactionOutput,
};
