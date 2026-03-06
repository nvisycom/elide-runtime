//! Convenience re-exports.

pub use crate::document::{AnyDocument, Document, Span, SpanEdit, SpanEditStream, SpanStream};
pub use crate::handler::{
    AnyAudio, AnyImage, AnyRich, AnyText, AudioData, AudioHandler, CsvData, CsvHandler, CsvLoader,
    CsvParams, CsvSpan, Handler, ImageData, ImageHandler, JpegHandler, JpegLoader, JpegParams,
    JsonData, JsonHandler, JsonIndent, JsonLoader, JsonParams, JsonPath, Loader, Mp3Handler,
    Mp3Loader, Mp3Params, PngHandler, PngLoader, PngParams, TextData, TextHandler, TxtHandler,
    TxtLoader, TxtParams, TxtSpan, WavHandler, WavLoader, WavParams,
};
#[cfg(feature = "docx")]
pub use crate::handler::{DocxHandler, DocxLoader, DocxParams};
#[cfg(feature = "html")]
pub use crate::handler::{HtmlData, HtmlHandler, HtmlLoader, HtmlParams, HtmlSpan};
#[cfg(feature = "pdf")]
pub use crate::handler::{PdfHandler, PdfImageSpan, PdfLoader, PdfParams, PdfTextSpan};
#[cfg(feature = "xlsx")]
pub use crate::handler::{XlsxHandler, XlsxLoader, XlsxParams};
pub use crate::transform::{
    AudioOutput, AudioRedact, AudioRedaction, ImageOutput, ImageRedact, ImageRedaction,
    ImageTransform, TextOutput, TextRedact, TextRedaction,
};
