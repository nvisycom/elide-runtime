//! Convenience re-exports.

pub use crate::document::{AnyDocument, Document, UniversalLoader};
pub use crate::handler::{
    AnyAudio, AnyImage, CsvData, CsvHandler, CsvLoader, CsvParams, CsvSpan, Handler, ImageData,
    JpegHandler, JpegLoader, JpegParams, JsonData, JsonHandler, JsonIndent, JsonLoader, JsonParams,
    JsonPath, Loader, Mp3Handler, Mp3Loader, Mp3Params, PngHandler, PngLoader, PngParams, Span,
    SpanEdit, TextData, TxtHandler, TxtLoader, TxtParams, TxtSpan, WavHandler, WavLoader,
    WavParams,
};
#[cfg(feature = "docx")]
pub use crate::handler::{DocxHandler, DocxLoader, DocxParams};
#[cfg(feature = "html")]
pub use crate::handler::{HtmlData, HtmlHandler, HtmlLoader, HtmlParams, HtmlSpan};
#[cfg(feature = "pdf")]
pub use crate::handler::{PdfHandler, PdfLoader, PdfParams, PdfSpan};
#[cfg(feature = "xlsx")]
pub use crate::handler::{XlsxHandler, XlsxLoader, XlsxParams};
pub use crate::stream::{SpanEditStream, SpanStream};
pub use crate::transform::{
    AudioHandler, AudioRedaction, AudioRedactionOutput, ImageHandler, ImageRedaction,
    ImageRedactionOutput, ImageTransform, TextHandler, TextRedaction, TextRedactionOutput,
};
