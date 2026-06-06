//! [`MemoryBuffer<M>`]: owned per-modality source bytes plus the
//! per-modality loader namespace and (where applicable) in-place
//! redaction.
//!
//! The pipeline-side traits ([`TextAt`], [`DataAt`]) need a
//! resolver to call into; every consumer that doesn't already have
//! a codec-backed implementation had to invent its own adapter.
//! `MemoryBuffer<M>` is the shipped adapter:
//!
//! - Owns its bytes via the underlying [`M::Data`] payload, so
//!   it can be both read from (`TextAt`, `DataAt`) and written
//!   to (per-modality `redact` methods) without borrow-checker
//!   gymnastics.
//! - Hosts the per-modality loader namespace
//!   (`MemoryBuffer::<Text>::from_text`,
//!   `MemoryBuffer::<Image>::from_bytes`, …) so users discover
//!   construction through one type instead of three.
//! - Excludes [`Tabular`] honestly via the [`Modality`] bound —
//!   there's no tabular in-memory loader to ship, no tabular
//!   anonymizer, and no `M::Data` to wrap.
//!
//! [`M::Data`]: Modality::Data
//! [`Tabular`]: nvisy_core::modality::Tabular
//! [`TextAt`]: nvisy_core::extraction::TextAt
//! [`DataAt`]: nvisy_core::extraction::DataAt
//! [`Modality`]: nvisy_core::Modality

use std::path::Path;

use bytes::Bytes;
use hipstr::HipStr;
use nvisy_core::extraction::{DataAt, TextAt};
use nvisy_core::modality::{
    Audio, AudioData, AudioLocation, Image, ImageData, ImageLocation, Modality, Text, TextData,
    TextLocation,
};
use nvisy_core::primitive::Dimensions;
use nvisy_core::redaction::{RedactAt, Redactions, TextReplacement};
use nvisy_core::{Error, Result};

const TARGET: &str = "nvisy_toolkit::ingestion::memory";

/// Owned in-memory source buffer for modality `M`.
///
/// Wraps the per-modality recognizer-input payload ([`M::Data`])
/// so callers have one type that loads, reads, and (for text)
/// rewrites.
///
/// [`M::Data`]: Modality::Data
#[derive(Debug, Clone)]
pub struct MemoryBuffer<M: Modality>(pub M::Data);

impl<M: Modality> MemoryBuffer<M> {
    /// Wrap an existing `M::Data` payload.
    pub fn new(data: M::Data) -> Self {
        Self(data)
    }

    /// Borrow the underlying payload by shared reference.
    pub fn data(&self) -> &M::Data {
        &self.0
    }

    /// Borrow the underlying payload by mutable reference.
    pub fn data_mut(&mut self) -> &mut M::Data {
        &mut self.0
    }

    /// Consume the buffer and return the owned payload.
    pub fn into_data(self) -> M::Data {
        self.0
    }
}

// `From<M::Data> for MemoryBuffer<M>` would conflict with the
// blanket `From<T> for T` whenever a downstream `M::Data` happens
// to equal `MemoryBuffer<M>`. Use [`MemoryBuffer::new`] directly.

// Text ───────────────────────────────────────────────────────────────

impl MemoryBuffer<Text> {
    /// Construct from an owned or borrowed string. Zero-copy when
    /// the input is `&'static str`.
    ///
    /// Named `from_text` rather than `from_str` to avoid shadowing
    /// the [`std::str::FromStr`] trait method, which would force
    /// callers into `let buf: MemoryBuffer<Text> = "...".parse()?`
    /// awkwardness.
    pub fn from_text(text: impl Into<HipStr<'static>>) -> Self {
        Self(TextData::new(text))
    }

    /// Read a UTF-8 text file into a buffer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::validation`] when the file cannot be read or
    /// its bytes are not valid UTF-8.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|err| {
            Error::validation(format!("read text file {}: {err}", path.display()), TARGET)
        })?;
        let text = String::from_utf8(bytes).map_err(|err| {
            Error::validation(
                format!("text file {} is not valid UTF-8: {err}", path.display()),
                TARGET,
            )
        })?;
        Ok(Self::from_text(text))
    }

    /// Borrow the wrapped text as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.text.as_str()
    }

    /// Apply an ordered batch of edits to the buffer in-place.
    ///
    /// Each entry is `(location, replacement)`. `Some(text)`
    /// substitutes the span with `text`; `None` deletes the span.
    /// Entries are sorted by `location.start` and applied from
    /// right to left so earlier offsets stay valid as later edits
    /// rewrite the buffer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::validation`] when any location is
    /// out-of-bounds for the current buffer or overlaps another
    /// entry. Overlapping redactions are a caller-side conflict
    /// (the deduplication phase should have resolved them upstream).
    pub fn redact(
        &mut self,
        edits: impl IntoIterator<Item = (TextLocation, Option<String>)>,
    ) -> Result<()> {
        let mut edits: Vec<(TextLocation, Option<String>)> = edits.into_iter().collect();
        if edits.is_empty() {
            return Ok(());
        }
        edits.sort_by_key(|(loc, _)| loc.start);

        let len = self.0.text.len();
        for (i, (loc, _)) in edits.iter().enumerate() {
            if loc.end > len {
                return Err(Error::validation(
                    format!(
                        "edit {i}: location {}..{} extends past buffer length {len}",
                        loc.start, loc.end
                    ),
                    TARGET,
                ));
            }
            if loc.start > loc.end {
                return Err(Error::validation(
                    format!("edit {i}: inverted location {}..{}", loc.start, loc.end),
                    TARGET,
                ));
            }
            if let Some((next_loc, _)) = edits.get(i + 1)
                && loc.end > next_loc.start
            {
                return Err(Error::validation(
                    format!(
                        "edit {i}: location {}..{} overlaps edit {} at {}..{}",
                        loc.start,
                        loc.end,
                        i + 1,
                        next_loc.start,
                        next_loc.end,
                    ),
                    TARGET,
                ));
            }
        }

        let mut out = String::with_capacity(len);
        out.push_str(self.as_str());
        for (loc, replacement) in edits.iter().rev() {
            let replacement = replacement.as_deref().unwrap_or("");
            out.replace_range(loc.start..loc.end, replacement);
        }
        self.0 = TextData::new(out);
        Ok(())
    }
}

#[async_trait::async_trait]
impl TextAt<Text> for MemoryBuffer<Text> {
    async fn text_at(&self, location: &TextLocation) -> Option<String> {
        self.as_str()
            .get(location.start..location.end)
            .map(str::to_owned)
    }
}

#[async_trait::async_trait]
impl DataAt<Text> for MemoryBuffer<Text> {
    async fn data_at(&self, location: &TextLocation) -> Option<TextData> {
        self.as_str()
            .get(location.start..location.end)
            .map(|s| TextData::new(s.to_owned()))
    }
}

#[async_trait::async_trait]
impl RedactAt<Text> for MemoryBuffer<Text> {
    /// Apply a batch of [`TextReplacement`] values back into the
    /// buffer. Flattens each replacement into the
    /// `(location, Option<String>)` shape the inherent [`redact`]
    /// method already accepts: `Substituted { value }` → `Some(value)`,
    /// `Removed` → `None`.
    ///
    /// [`redact`]: Self::redact
    async fn redact_at(&mut self, redactions: Redactions<Text>) -> Result<()> {
        let edits = redactions.into_items().into_iter().map(|(loc, repl)| {
            let replacement = match repl {
                TextReplacement::Substituted { value } => Some(value),
                TextReplacement::Removed => None,
            };
            (loc, replacement)
        });
        self.redact(edits)
    }
}

// Image ──────────────────────────────────────────────────────────────

impl MemoryBuffer<Image> {
    /// Wrap encoded image bytes alongside their pixel dimensions.
    pub fn from_bytes(bytes: impl Into<Bytes>, dims: Dimensions) -> Self {
        Self(ImageData::new(bytes, dims))
    }

    /// Read an image file into a buffer. The file's bytes are
    /// stored as-is; callers supply the pixel `dims` because the
    /// toolkit does not depend on an image-decoding crate at this
    /// layer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::validation`] when the file cannot be read.
    pub fn from_file(path: impl AsRef<Path>, dims: Dimensions) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|err| {
            Error::validation(format!("read image file {}: {err}", path.display()), TARGET)
        })?;
        Ok(Self::from_bytes(bytes, dims))
    }

    /// Borrow the encoded bytes.
    pub fn bytes(&self) -> &Bytes {
        &self.0.bytes
    }

    /// Pixel dimensions of the wrapped image.
    pub fn dimensions(&self) -> Dimensions {
        self.0.dims
    }
}

#[async_trait::async_trait]
impl DataAt<Image> for MemoryBuffer<Image> {
    async fn data_at(&self, _location: &ImageLocation) -> Option<ImageData> {
        // The whole image is the only payload — image locations
        // index into pixel space, not byte ranges, so there's no
        // cheap sub-slice.
        Some(self.0.clone())
    }
}

// Audio ──────────────────────────────────────────────────────────────

impl MemoryBuffer<Audio> {
    /// Wrap encoded audio bytes alongside the source filename
    /// (informational; some STT backends pass it through).
    pub fn from_bytes(bytes: impl Into<Bytes>, filename: impl Into<HipStr<'static>>) -> Self {
        Self(AudioData::new(bytes).with_filename(filename))
    }

    /// Read an audio file into a buffer. The file name is recorded
    /// from the path's file component when present.
    ///
    /// # Errors
    ///
    /// Returns [`Error::validation`] when the file cannot be read.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|err| {
            Error::validation(format!("read audio file {}: {err}", path.display()), TARGET)
        })?;
        let mut data = AudioData::new(bytes);
        if let Some(filename) = path.file_name().map(|s| s.to_string_lossy().into_owned()) {
            data = data.with_filename(filename);
        }
        Ok(Self(data))
    }

    /// Borrow the encoded bytes.
    pub fn bytes(&self) -> &Bytes {
        &self.0.bytes
    }
}

#[async_trait::async_trait]
impl DataAt<Audio> for MemoryBuffer<Audio> {
    async fn data_at(&self, _location: &AudioLocation) -> Option<AudioData> {
        // Audio locations are time spans; the full buffer is what
        // an anonymizer receives. Backends slice by time internally.
        Some(self.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_from_str_round_trips() {
        let buf = MemoryBuffer::<Text>::from_text("hello world");
        assert_eq!(buf.as_str(), "hello world");
    }

    #[test]
    fn text_redact_substitutes_in_place() {
        let mut buf = MemoryBuffer::<Text>::from_text("alice@example.test and bob@example.test");
        buf.redact([
            (TextLocation::new(0, 18), Some("[EMAIL]".into())),
            (TextLocation::new(23, 39), Some("[EMAIL]".into())),
        ])
        .expect("non-overlapping edits");
        assert_eq!(buf.as_str(), "[EMAIL] and [EMAIL]");
    }

    #[test]
    fn text_redact_with_none_deletes_span() {
        let mut buf = MemoryBuffer::<Text>::from_text("keep <drop me> keep");
        buf.redact([(TextLocation::new(5, 14), None)]).unwrap();
        assert_eq!(buf.as_str(), "keep  keep");
    }

    #[test]
    fn text_redact_sorts_unordered_input() {
        let mut buf = MemoryBuffer::<Text>::from_text("aaaa bbbb cccc");
        buf.redact([
            (TextLocation::new(10, 14), Some("CCCC".into())),
            (TextLocation::new(0, 4), Some("AAAA".into())),
            (TextLocation::new(5, 9), Some("BBBB".into())),
        ])
        .unwrap();
        assert_eq!(buf.as_str(), "AAAA BBBB CCCC");
    }

    #[test]
    fn text_redact_rejects_out_of_bounds() {
        let mut buf = MemoryBuffer::<Text>::from_text("short");
        let err = buf
            .redact([(TextLocation::new(0, 99), Some("x".into()))])
            .unwrap_err();
        assert!(format!("{err}").contains("past buffer length"));
    }

    #[test]
    fn text_redact_rejects_overlapping_edits() {
        let mut buf = MemoryBuffer::<Text>::from_text("aaaaaaaaaa");
        let err = buf
            .redact([
                (TextLocation::new(0, 5), Some("x".into())),
                (TextLocation::new(3, 7), Some("y".into())),
            ])
            .unwrap_err();
        assert!(format!("{err}").contains("overlaps"));
    }

    #[tokio::test]
    async fn text_redact_at_trait_applies_substituted_and_removed() {
        let mut buf = MemoryBuffer::<Text>::from_text(
            "alice@example.test and <drop me> and 4111111111111111",
        );
        let mut rs = Redactions::<Text>::new();
        rs.push(
            TextLocation::new(0, 18),
            TextReplacement::substituted("[EMAIL]"),
        );
        rs.push(TextLocation::new(23, 32), TextReplacement::Removed);
        rs.push(
            TextLocation::new(37, 53),
            TextReplacement::substituted("[CARD]"),
        );
        buf.redact_at(rs).await.unwrap();
        assert_eq!(buf.as_str(), "[EMAIL] and  and [CARD]");
    }

    #[test]
    fn image_from_bytes_records_dims() {
        let buf = MemoryBuffer::<Image>::from_bytes(vec![0u8; 10], Dimensions::new(8, 8));
        assert_eq!(buf.bytes().len(), 10);
        assert_eq!(buf.dimensions(), Dimensions::new(8, 8));
    }

    #[test]
    fn audio_from_bytes_records_filename() {
        let buf = MemoryBuffer::<Audio>::from_bytes(vec![0u8; 4], "sample.wav");
        assert_eq!(buf.bytes().len(), 4);
        assert_eq!(buf.0.filename.as_deref(), Some("sample.wav"));
    }
}
