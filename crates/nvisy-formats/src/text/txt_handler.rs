//! Plain-text handler: holds loaded text content and streams it
//! line-by-line via [`Handle<Text>`], with random-access reads /
//! redactions via [`IndexedHandle<Text>`].
//!
//! The handler stores the text as a vector of lines together with a
//! trailing-newline flag so the original file can be reconstructed
//! byte-for-byte after edits.

use std::sync::Arc;

use async_trait::async_trait;
use nvisy_codec::core::{Chunk, Handle, IndexedHandle};
use nvisy_codec::handler::Handler;
use nvisy_codec::{Format, FormatId, LoaderAdapter};
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::modality::{ModalityKind, Text, TextData, TextLocation};
use nvisy_core::redaction::{Redactions, TextReplacement};

use super::{TxtLoader, redact};

const TARGET: &str = "txt-handler";

/// Stable [`FormatId`] for the plain-text codec.
pub const FORMAT_ID: FormatId = FormatId::from_static("nvisy.text.txt");

/// [`Format`] descriptor registered into [`nvisy_codec::CodecRegistry`].
pub fn format() -> Format {
    Format {
        id: FORMAT_ID.clone(),
        modality: ModalityKind::Text,
        extensions: vec!["txt".into(), "log".into()],
        content_types: vec!["text/plain".into()],
        loader: Arc::new(LoaderAdapter::new(TxtLoader::default())),
    }
}

/// Handler for loaded plain-text content. Each line is independently
/// addressable via [`TextLocation`].
///
/// `line_starts` is a cumulative-offset index maintained alongside
/// `lines`: `line_starts[i]` is the byte position of line `i` in the
/// serialized output, and `line_starts[lines.len()]` is the total
/// length sentinel. Random-access [`IndexedHandle::read`] and
/// [`IndexedHandle::redact`] resolve a byte offset to a line in
/// `O(log N)` instead of rebuilding the table on every call.
#[derive(Debug)]
pub struct TxtHandler {
    source: ContentSource,
    lines: Vec<String>,
    line_starts: Vec<usize>,
    trailing_newline: bool,
    cursor: usize,
}

impl Handler for TxtHandler {
    fn format(&self) -> FormatId {
        FORMAT_ID.clone()
    }

    fn source(&self) -> &ContentSource {
        &self.source
    }

    #[tracing::instrument(name = "txt.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<ContentData, Error> {
        let mut out = self.lines.join("\n");
        if self.trailing_newline && !self.lines.is_empty() {
            out.push('\n');
        }
        let bytes = out.into_bytes();
        tracing::Span::current().record("output_bytes", bytes.len());
        let source = ContentSource::new().with_parent(&self.source);
        Ok(ContentData::new(source, bytes.into()))
    }
}

#[async_trait]
impl Handle<Text> for TxtHandler {
    async fn next_chunk(&mut self) -> Result<Option<Chunk<Text>>, Error> {
        if self.cursor >= self.lines.len() {
            return Ok(None);
        }
        let i = self.cursor;
        let start = self.line_starts[i];
        let end = self.line_starts[i + 1] - 1; // strip the implicit '\n' separator
        let line = &self.lines[i];
        self.cursor += 1;
        Ok(Some(Chunk {
            location: TextLocation {
                start,
                end,
                ..Default::default()
            },
            data: TextData::from(line.as_str()),
            embed: None,
        }))
    }
}

#[async_trait]
impl IndexedHandle<Text> for TxtHandler {
    async fn read(&self, location: &TextLocation) -> Result<Option<TextData>, Error> {
        let Some(i) = self.line_for(location.start) else {
            return Ok(None);
        };
        let line_start = self.line_starts[i];
        let line_end = self.line_starts[i + 1] - 1;
        if location.end > line_end {
            return Ok(None); // crosses a line boundary
        }
        let local_start = location.start - line_start;
        let local_end = location.end - line_start;
        Ok(self.lines[i]
            .get(local_start..local_end)
            .map(TextData::from))
    }

    async fn redact(&mut self, redactions: Redactions<Text>) -> Result<(), Error> {
        // Apply right-to-left so each edit's length delta doesn't
        // invalidate earlier locations.
        let mut items = redactions.into_items();
        items.sort_by_key(|(loc, _)| std::cmp::Reverse(loc.start));
        for (location, replacement) in items {
            self.redact_one(&location, replacement)?;
        }
        Ok(())
    }
}

impl TxtHandler {
    /// Create a new handler from lines and a trailing-newline flag.
    pub fn new(lines: Vec<String>, trailing_newline: bool) -> Self {
        let line_starts = compute_line_starts(&lines);
        Self {
            source: ContentSource::new(),
            lines,
            line_starts,
            trailing_newline,
            cursor: 0,
        }
    }

    /// Attach a content source for lineage tracking.
    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }

    /// All lines in the document.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// A specific line by 0-based index.
    pub fn line(&self, index: usize) -> Option<&str> {
        self.lines.get(index).map(String::as_str)
    }

    /// Whether the original source had a trailing newline.
    pub fn trailing_newline(&self) -> bool {
        self.trailing_newline
    }

    /// Total number of lines.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether the document has no lines.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Rewind the streaming cursor to the start of the document.
    pub fn rewind(&mut self) {
        self.cursor = 0;
    }

    /// Line index containing `byte_offset`, or `None` if the offset
    /// is past the end of the document.
    fn line_for(&self, byte_offset: usize) -> Option<usize> {
        match self.line_starts.binary_search(&byte_offset) {
            Ok(i) if i < self.lines.len() => Some(i),
            Ok(_) => None, // landed on the trailing sentinel
            Err(i) if i > 0 && i <= self.lines.len() => Some(i - 1),
            _ => None,
        }
    }

    /// Shift every `line_starts[j]` for `j > i` by `delta`. Called
    /// after a redaction changes the length of line `i`.
    fn shift_starts_after(&mut self, i: usize, delta: isize) {
        if delta == 0 {
            return;
        }
        for s in &mut self.line_starts[i + 1..] {
            *s = (*s as isize + delta) as usize;
        }
    }

    fn redact_one(
        &mut self,
        location: &TextLocation,
        replacement: TextReplacement,
    ) -> Result<(), Error> {
        let Some(i) = self.line_for(location.start) else {
            return Ok(());
        };
        let line_start = self.line_starts[i];
        let line_end = self.line_starts[i + 1] - 1;
        if location.end > line_end {
            return Ok(());
        }
        let local_start = location.start - line_start;
        let local_end = location.end - line_start;
        let value = replacement.replacement_value().unwrap_or_default();
        let before_len = self.lines[i].len();
        redact::replace_range(&mut self.lines[i], value, local_start, local_end, TARGET)?;
        let after_len = self.lines[i].len();
        self.shift_starts_after(i, after_len as isize - before_len as isize);
        Ok(())
    }
}

fn compute_line_starts(lines: &[String]) -> Vec<usize> {
    let mut starts = Vec::with_capacity(lines.len() + 1);
    let mut offset = 0usize;
    for line in lines {
        starts.push(offset);
        offset += line.len() + 1; // +1 for the implicit '\n' separator
    }
    starts.push(offset);
    starts
}

#[cfg(test)]
mod tests {
    use nvisy_core::Error;

    use super::*;

    fn handler(text: &str) -> TxtHandler {
        let trailing_newline = text.ends_with('\n');
        let lines = text.lines().map(String::from).collect();
        TxtHandler::new(lines, trailing_newline)
    }

    #[tokio::test]
    async fn stream_yields_each_line() -> Result<(), Error> {
        let mut h = handler("hello\nworld\n");
        let first = h.next_chunk().await?.unwrap();
        assert_eq!(first.location.start, 0);
        assert_eq!(first.location.end, 5);
        assert_eq!(first.data.as_str(), "hello");
        let second = h.next_chunk().await?.unwrap();
        assert_eq!(second.location.start, 6);
        assert_eq!(second.location.end, 11);
        assert_eq!(second.data.as_str(), "world");
        assert!(h.next_chunk().await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn stream_single_line_no_newline() -> Result<(), Error> {
        let mut h = handler("no newline");
        let chunk = h.next_chunk().await?.unwrap();
        assert_eq!(chunk.location.start, 0);
        assert_eq!(chunk.location.end, 10);
        assert!(!h.trailing_newline());
        assert!(h.next_chunk().await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn read_returns_line() -> Result<(), Error> {
        let h = handler("hello\nworld\n");
        let loc = TextLocation {
            start: 6,
            end: 11,
            ..Default::default()
        };
        assert_eq!(h.read(&loc).await?.unwrap().as_str(), "world");
        Ok(())
    }

    #[tokio::test]
    async fn read_cross_line_returns_none() -> Result<(), Error> {
        let h = handler("hello\nworld\n");
        let loc = TextLocation {
            start: 3,
            end: 8,
            ..Default::default()
        };
        assert!(h.read(&loc).await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn redact_replaces_whole_line() -> Result<(), Error> {
        let mut h = handler("hello\nworld\n");
        let mut rs = Redactions::new();
        rs.push(
            TextLocation {
                start: 6,
                end: 11,
                ..Default::default()
            },
            TextReplacement::substituted("[REDACTED]"),
        );
        h.redact(rs).await?;
        assert_eq!(h.lines(), &["hello", "[REDACTED]"]);
        Ok(())
    }

    #[tokio::test]
    async fn redact_substring_within_line() -> Result<(), Error> {
        let mut h = handler("hello world");
        let mut rs = Redactions::new();
        rs.push(
            TextLocation {
                start: 6,
                end: 11,
                ..Default::default()
            },
            TextReplacement::substituted("[X]"),
        );
        h.redact(rs).await?;
        assert_eq!(h.lines(), &["hello [X]"]);
        Ok(())
    }

    #[tokio::test]
    async fn redact_multiple_lines_left_to_right_input() -> Result<(), Error> {
        // Batch is right-to-left-sorted internally; client passes any order.
        let mut h = handler("alpha\nbravo\ncharlie\n");
        let mut rs = Redactions::new();
        rs.push(
            TextLocation {
                start: 0,
                end: 5,
                ..Default::default()
            },
            TextReplacement::substituted("[A]"),
        );
        rs.push(
            TextLocation {
                start: 12,
                end: 19,
                ..Default::default()
            },
            TextReplacement::substituted("[C]"),
        );
        h.redact(rs).await?;
        assert_eq!(h.lines(), &["[A]", "bravo", "[C]"]);
        Ok(())
    }

    #[tokio::test]
    async fn redact_unknown_location_skipped() -> Result<(), Error> {
        let mut h = handler("one line");
        let mut rs = Redactions::new();
        rs.push(
            TextLocation {
                start: 999,
                end: 1000,
                ..Default::default()
            },
            TextReplacement::substituted("nope"),
        );
        h.redact(rs).await?;
        assert_eq!(h.lines(), &["one line"]);
        Ok(())
    }

    #[test]
    fn encode_with_trailing_newline() -> Result<(), Error> {
        let h = handler("hello\nworld\n");
        let content = h.encode()?;
        assert_eq!(content.as_bytes(), b"hello\nworld\n");
        Ok(())
    }

    #[test]
    fn encode_without_trailing_newline() -> Result<(), Error> {
        let h = handler("no newline");
        let content = h.encode()?;
        assert_eq!(content.as_bytes(), b"no newline");
        Ok(())
    }

    #[test]
    fn line_starts_initialised_correctly() {
        let h = handler("hello\nworld\n");
        // hello [0..5], world [6..11], total = 12 (sentinel)
        assert_eq!(h.line_starts, vec![0, 6, 12]);
    }

    #[tokio::test]
    async fn line_starts_shift_after_shrink() -> Result<(), Error> {
        let mut h = handler("hello\nworld\n");
        let mut rs = Redactions::new();
        rs.push(
            TextLocation {
                start: 0,
                end: 5,
                ..Default::default()
            },
            TextReplacement::substituted("[X]"),
        );
        h.redact(rs).await?;
        // line 0 shrank from 5 → 3, so line 1's start shifted by -2.
        assert_eq!(h.line_starts, vec![0, 4, 10]);
        assert_eq!(h.lines(), &["[X]", "world"]);
        Ok(())
    }
}
