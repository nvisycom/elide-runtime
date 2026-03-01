//! Convenience re-exports.

pub use crate::document::Document;
pub use crate::handler::{
    AnyAudio, AnyImage, AudioData, AudioHandler, CsvData, CsvHandler, CsvLoader, CsvParams, CsvSpan, Handler,
    ImageData, ImageHandler, JpegHandler, JpegLoader, JpegParams, JsonData, JsonHandler,
    JsonIndent, JsonLoader, JsonParams, JsonPath, Loader, Mp3Handler, Mp3Loader, Mp3Params,
    PngHandler, PngLoader, PngParams, Span, SpanEdit, SpanEditStream, SpanStream, TextData,
    TextHandler, TxtHandler, TxtLoader, TxtParams, TxtSpan, WavHandler, WavLoader, WavParams,
};
#[cfg(feature = "docx")]
pub use crate::handler::{DocxHandler, DocxLoader, DocxParams};
#[cfg(feature = "html")]
pub use crate::handler::{HtmlData, HtmlHandler, HtmlLoader, HtmlParams, HtmlSpan};
#[cfg(feature = "pdf")]
pub use crate::handler::{PdfHandler, PdfLoader, PdfParams, PdfTextSpan, PdfImageSpan};
#[cfg(feature = "xlsx")]
pub use crate::handler::{XlsxHandler, XlsxLoader, XlsxParams};
pub use crate::transform::{
    AudioRedact, AudioRedaction, AudioRedactionOutput, ImageRedact, ImageRedaction,
    ImageRedactionOutput, ImageTransform, TextRedact, TextRedaction, TextRedactionOutput,
};
