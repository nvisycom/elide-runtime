//! Convenience re-exports.

pub use crate::handler::{
    AudioData, AudioHandler, AudioSpanId, BoxedAudioHandler, BoxedImageHandler, BoxedRichHandler,
    BoxedTextHandler, CsvData, CsvHandler, CsvLoader, CsvParams, CsvSpan, Handler, ImageData,
    ImageHandler, ImageSpanId, JpegHandler, JpegLoader, JpegParams, JsonData, JsonHandler,
    JsonIndent, JsonLoader, JsonParams, JsonPath, Loader, Mp3Handler, Mp3Loader, Mp3Params,
    PngHandler, PngLoader, PngParams, TextData, TextHandler, TextSpanId, TxtHandler, TxtLoader,
    TxtParams, TxtSpan, WavHandler, WavLoader, WavParams,
};
#[cfg(feature = "docx")]
pub use crate::handler::{DocxLoader, DocxParams};
#[cfg(feature = "html")]
pub use crate::handler::{HtmlData, HtmlHandler, HtmlLoader, HtmlParams, HtmlSpan};
#[cfg(feature = "pdf")]
pub use crate::handler::{PdfLoader, PdfParams, RichTextHandler, RichTextSpan};
#[cfg(feature = "xlsx")]
pub use crate::handler::{XlsxHandler, XlsxLoader, XlsxParams};
pub use crate::transform::{
    AudioOutput, AudioRedaction, AudioTransform, ImageOutput, ImageRedaction, ImageTransform,
    TextOutput, TextRedaction, TextTransform,
};
pub use crate::{Document, Span, SpanStream};
