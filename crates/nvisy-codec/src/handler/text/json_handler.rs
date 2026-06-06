//! JSON handler: holds parsed JSON content and streams its string
//! leaves + object keys via [`Handle<Text>`], with random-access
//! reads / redactions via [`IndexedHandle<Text>`].
//!
//! The handler stores the parsed [`Value`] tree together with the
//! original indentation style and trailing-newline flag, so the source
//! file can be reconstructed with identical whitespace after edits.
//!
//! [`Handle::next_chunk`] yields string-typed JSON leaves and object
//! keys, addressed by [`TextLocation`] byte offsets in the serialized
//! form. Locations are resolved lazily into a memo of [`LocatedSpan`]s
//! that is invalidated whenever a redaction edits the tree (because
//! the serialization — and therefore every byte offset — changes).
//!
//! [`Value`]: serde_json::Value

use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use nvisy_core::Error;
use nvisy_core::modality::{ModalityKind, Text, TextData, TextLocation};
use nvisy_core::redaction::Redactions;
use serde::{Deserialize, Serialize};

use super::{JsonLoader, redact};
use crate::content::{ContentData, ContentSource};
use crate::core::{Chunk, Handle, Handler, IndexedHandle};
use crate::{Format, FormatId, LoaderAdapter};

const DEFAULT_INDENT: NonZeroU32 = NonZeroU32::new(2).unwrap();
const TARGET: &str = "json-handler";

/// Stable [`FormatId`] for the JSON codec.
pub const FORMAT_ID: FormatId = FormatId::from_static("nvisy.text.json");

/// [`Format`] descriptor registered into [`crate::CodecRegistry`].
pub fn format() -> Format {
    Format {
        id: FORMAT_ID.clone(),
        modality: ModalityKind::Text,
        extensions: vec!["json".into()],
        content_types: vec!["application/json".into()],
        loader: Arc::new(LoaderAdapter::new(JsonLoader::default())),
    }
}

/// [RFC 6901] JSON Pointer identifying a span within a JSON document.
///
/// Used internally by [`JsonHandler`] for tree navigation.
///
/// [RFC 6901]: https://www.rfc-editor.org/rfc/rfc6901
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct JsonPath {
    pointer: String,
    key_of: bool,
}

impl JsonPath {
    fn value(pointer: impl Into<String>) -> Self {
        Self {
            pointer: pointer.into(),
            key_of: false,
        }
    }

    fn key(pointer: impl Into<String>) -> Self {
        Self {
            pointer: pointer.into(),
            key_of: true,
        }
    }
}

/// Indentation style detected in the original JSON source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JsonIndent {
    /// No whitespace between tokens (`{"a":1}`).
    Compact,
    /// N-space indentation.
    Spaces(NonZeroU32),
    /// Tab indentation.
    Tab,
}

impl JsonIndent {
    /// Two-space indentation.
    pub fn two_spaces() -> Self {
        Self::Spaces(NonZeroU32::new(2).unwrap())
    }

    /// Four-space indentation.
    pub fn four_spaces() -> Self {
        Self::Spaces(NonZeroU32::new(4).unwrap())
    }
}

/// Parsed JSON content together with its original formatting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonData {
    pub value: serde_json::Value,
    pub indent: JsonIndent,
    pub trailing_newline: bool,
}

impl Default for JsonData {
    fn default() -> Self {
        Self {
            value: serde_json::Value::Null,
            indent: JsonIndent::Spaces(DEFAULT_INDENT),
            trailing_newline: true,
        }
    }
}

/// A located JSON span with its tree path, unescaped text, and the
/// byte offsets it occupies in the serialized form.
#[derive(Debug, Clone)]
struct LocatedSpan {
    path: JsonPath,
    text: String,
    start: usize,
    end: usize,
}

/// Handler for loaded JSON content.
///
/// `spans` is a lazily-computed memo of every string leaf + object
/// key in the parsed tree, paired with its byte offset in the
/// serialized output. Cleared on every redaction because serialization
/// — and therefore every byte offset — changes when the tree mutates.
#[derive(Debug)]
pub struct JsonHandler {
    source: ContentSource,
    data: JsonData,
    spans: Mutex<Option<Vec<LocatedSpan>>>,
    cursor: usize,
}

impl Handler for JsonHandler {
    fn format(&self) -> FormatId {
        FORMAT_ID.clone()
    }

    fn source(&self) -> &ContentSource {
        &self.source
    }

    #[tracing::instrument(name = "json.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<ContentData, Error> {
        let mut bytes = self.serialize_to_bytes()?;
        if self.data.trailing_newline {
            bytes.push(b'\n');
        }
        tracing::Span::current().record("output_bytes", bytes.len());
        let source = ContentSource::new().with_parent(&self.source);
        Ok(ContentData::new(source, bytes.into()))
    }
}

#[async_trait]
impl Handle<Text> for JsonHandler {
    async fn next_chunk(&mut self) -> Result<Option<Chunk<Text>>, Error> {
        let chunk = self.with_spans(|spans| {
            if self.cursor >= spans.len() {
                return None;
            }
            let s = &spans[self.cursor];
            Some(Chunk {
                location: TextLocation {
                    start: s.start,
                    end: s.end,
                    ..Default::default()
                },
                data: TextData::from(s.text.as_str()),
                embed: None,
            })
        });
        if chunk.is_some() {
            self.cursor += 1;
        }
        Ok(chunk)
    }
}

#[async_trait]
impl IndexedHandle<Text> for JsonHandler {
    async fn read(&self, location: &TextLocation) -> Result<Option<TextData>, Error> {
        Ok(self.with_spans(|spans| {
            spans
                .iter()
                .find(|s| s.start == location.start && s.end == location.end)
                .map(|s| TextData::from(s.text.as_str()))
        }))
    }

    async fn redact(&mut self, redactions: Redactions<Text>) -> Result<(), Error> {
        // Resolve every location against the current serialization in
        // one pass, then apply each mutation against the tree. Tree
        // mutations don't invalidate already-resolved tree paths, so
        // ordering is irrelevant once locations are turned into paths.
        let resolved: Vec<(JsonPath, String)> = self.with_spans(|spans| {
            redactions
                .into_items()
                .into_iter()
                .filter_map(|(loc, replacement)| {
                    let s = spans
                        .iter()
                        .find(|s| loc.start >= s.start && loc.end <= s.end)?;
                    let value = replacement.replacement_value().unwrap_or_default();
                    let start = loc.start - s.start;
                    let end = loc.end - s.start;
                    let mut content = s.text.clone();
                    redact::replace_range(&mut content, value, start, end, TARGET).ok()?;
                    Some((s.path.clone(), content))
                })
                .collect()
        });
        for (path, content) in resolved {
            self.apply_edit(&path, content)?;
        }
        self.invalidate_spans();
        Ok(())
    }
}

impl JsonHandler {
    /// Create a new handler from parsed JSON data.
    pub fn new(data: JsonData) -> Self {
        Self {
            source: ContentSource::new(),
            data,
            spans: Mutex::new(None),
            cursor: 0,
        }
    }

    /// Attach a content source for lineage tracking.
    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }

    /// Access the underlying JSON value.
    pub fn value(&self) -> &serde_json::Value {
        &self.data.value
    }

    /// Mutable access to the underlying JSON value. Invalidates the
    /// span memo since callers may mutate the tree out-of-band.
    pub fn value_mut(&mut self) -> &mut serde_json::Value {
        self.invalidate_spans();
        &mut self.data.value
    }

    /// Detected indentation style.
    pub fn indent(&self) -> JsonIndent {
        self.data.indent
    }

    /// Whether the original source had a trailing newline.
    pub fn trailing_newline(&self) -> bool {
        self.data.trailing_newline
    }

    /// Rewind the streaming cursor to the start of the document.
    pub fn rewind(&mut self) {
        self.cursor = 0;
    }

    /// Run `f` with the lazily-populated span memo. Populates on
    /// first call after construction or after invalidation.
    fn with_spans<R>(&self, f: impl FnOnce(&[LocatedSpan]) -> R) -> R {
        let mut guard = self.spans.lock().expect("span memo lock");
        if guard.is_none() {
            *guard = Some(self.locate_spans());
        }
        f(guard.as_deref().expect("just populated"))
    }

    /// Clear the span memo. Called after any mutation that changes
    /// the serialized form.
    fn invalidate_spans(&mut self) {
        *self.spans.lock().expect("span memo lock") = None;
        self.cursor = 0;
    }

    /// Apply a redaction's replacement text at the given JSON path.
    /// For value paths, parse the content as JSON if possible; for
    /// keys, rename in the parent object.
    fn apply_edit(&mut self, path: &JsonPath, content: String) -> Result<(), Error> {
        if path.key_of {
            rename_key(&mut self.data.value, &path.pointer, &content)?;
        } else {
            let target = self.data.value.pointer_mut(&path.pointer).ok_or_else(|| {
                Error::validation(format!("JSON pointer not found: {}", path.pointer), TARGET)
            })?;
            if target.is_string() {
                *target = serde_json::Value::String(content);
            } else {
                *target =
                    serde_json::from_str(&content).unwrap_or(serde_json::Value::String(content));
            }
        }
        Ok(())
    }

    /// Walk the parsed tree, serialize once, and find each span's
    /// byte offsets via a monotonic cursor (avoids duplicate-value
    /// ambiguity).
    fn locate_spans(&self) -> Vec<LocatedSpan> {
        let serialized = self.serialize_to_string();
        let tree_spans: Vec<_> = JsonSpanIter::new(&self.data.value).collect();
        let mut result = Vec::with_capacity(tree_spans.len());
        let mut cursor = 0usize;

        for ts in &tree_spans {
            let text = match &ts.value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let needle = if ts.value.is_string() {
                format!("\"{}\"", json_escape(&text))
            } else {
                text.clone()
            };
            if let Some(rel) = serialized[cursor..].find(&needle) {
                let start = cursor + rel;
                let end = start + needle.len();
                result.push(LocatedSpan {
                    path: ts.path.clone(),
                    text,
                    start,
                    end,
                });
                cursor = end;
            }
        }
        result
    }

    fn serialize_to_string(&self) -> String {
        self.serialize_to_bytes()
            .map(|b| String::from_utf8(b).unwrap_or_default())
            .unwrap_or_default()
    }

    fn serialize_to_bytes(&self) -> Result<Vec<u8>, Error> {
        match self.data.indent {
            JsonIndent::Compact => serde_json::to_vec(&self.data.value)
                .map_err(|e| Error::validation(format!("JSON encode error: {e}"), TARGET)),
            JsonIndent::Spaces(n) => {
                let indent = " ".repeat(n.get() as usize);
                let mut buf = Vec::new();
                let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
                let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
                serde::Serialize::serialize(&self.data.value, &mut ser)
                    .map_err(|e| Error::validation(format!("JSON encode error: {e}"), TARGET))?;
                Ok(buf)
            }
            JsonIndent::Tab => {
                let mut buf = Vec::new();
                let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
                let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
                serde::Serialize::serialize(&self.data.value, &mut ser)
                    .map_err(|e| Error::validation(format!("JSON encode error: {e}"), TARGET))?;
                Ok(buf)
            }
        }
    }
}

/// Escape a string for JSON matching (backslash and quote).
fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Internal span from the JSON tree walker.
struct JsonTreeSpan {
    path: JsonPath,
    value: serde_json::Value,
}

/// Depth-first iterator over JSON string leaves and object keys.
struct JsonSpanIter {
    stack: Vec<JsonTreeSpan>,
}

impl JsonSpanIter {
    fn new(value: &serde_json::Value) -> Self {
        let mut stack = Vec::new();
        Self::push_value(&mut stack, "", value);
        stack.reverse();
        Self { stack }
    }

    fn push_value(stack: &mut Vec<JsonTreeSpan>, pointer: &str, value: &serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, val) in map {
                    let escaped_key = key.replace('~', "~0").replace('/', "~1");
                    let child_ptr = format!("{pointer}/{escaped_key}");
                    stack.push(JsonTreeSpan {
                        path: JsonPath::key(&child_ptr),
                        value: serde_json::Value::String(key.clone()),
                    });
                    Self::push_value(stack, &child_ptr, val);
                }
            }
            serde_json::Value::Array(arr) => {
                for (i, val) in arr.iter().enumerate() {
                    let child_ptr = format!("{pointer}/{i}");
                    Self::push_value(stack, &child_ptr, val);
                }
            }
            _ => {
                stack.push(JsonTreeSpan {
                    path: JsonPath::value(pointer),
                    value: value.clone(),
                });
            }
        }
    }
}

impl Iterator for JsonSpanIter {
    type Item = JsonTreeSpan;

    fn next(&mut self) -> Option<Self::Item> {
        self.stack.pop()
    }
}

/// Rename an object key at the given JSON pointer path.
fn rename_key(root: &mut serde_json::Value, pointer: &str, new_key: &str) -> Result<(), Error> {
    let (parent_ptr, old_key_segment) = pointer
        .rsplit_once('/')
        .ok_or_else(|| Error::validation(format!("cannot rename root: {pointer}"), TARGET))?;
    let old_key = old_key_segment.replace("~1", "/").replace("~0", "~");
    let parent = if parent_ptr.is_empty() {
        root
    } else {
        root.pointer_mut(parent_ptr).ok_or_else(|| {
            Error::validation(format!("parent pointer not found: {parent_ptr}"), TARGET)
        })?
    };
    let obj = parent
        .as_object_mut()
        .ok_or_else(|| Error::validation("parent is not an object", TARGET))?;
    if let Some(value) = obj.remove(&old_key) {
        obj.insert(new_key.to_string(), value);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use nvisy_core::Error;
    use nvisy_core::redaction::TextReplacement;

    use super::*;

    fn compact_handler(json: &str) -> JsonHandler {
        JsonHandler::new(JsonData {
            value: serde_json::from_str(json).unwrap(),
            indent: JsonIndent::Compact,
            trailing_newline: false,
        })
    }

    #[tokio::test]
    async fn stream_yields_string_leaves_and_keys() -> Result<(), Error> {
        let mut h = compact_handler(r#"{"name":"Alice","age":30}"#);
        let mut count = 0;
        while h.next_chunk().await?.is_some() {
            count += 1;
        }
        // 2 keys + 2 leaves
        assert_eq!(count, 4);
        Ok(())
    }

    #[tokio::test]
    async fn duplicate_values_get_distinct_offsets() -> Result<(), Error> {
        let mut h = compact_handler(r#"{"a":"same","b":"same"}"#);
        let mut same_offsets = Vec::new();
        while let Some(chunk) = h.next_chunk().await? {
            if chunk.data.as_str() == "same" {
                same_offsets.push(chunk.location.start);
            }
        }
        assert_eq!(same_offsets.len(), 2);
        assert_ne!(same_offsets[0], same_offsets[1]);
        Ok(())
    }

    #[tokio::test]
    async fn read_returns_string() -> Result<(), Error> {
        let mut h = compact_handler(r#"{"name":"Alice"}"#);
        let mut found = false;
        while let Some(chunk) = h.next_chunk().await? {
            if h.read(&chunk.location)
                .await?
                .map(|d| d.as_str().to_owned())
                == Some("Alice".to_owned())
            {
                found = true;
            }
        }
        assert!(found);
        Ok(())
    }

    #[tokio::test]
    async fn read_rejects_arbitrary_offsets() -> Result<(), Error> {
        let h = compact_handler(r#"{"name":"Alice"}"#);
        let bogus = TextLocation {
            start: 3,
            end: 7,
            ..Default::default()
        };
        assert!(h.read(&bogus).await?.is_none());
        Ok(())
    }

    #[test]
    fn encode_compact() -> Result<(), Error> {
        let h = compact_handler(r#"{"a":1}"#);
        let content = h.encode()?;
        assert_eq!(content.as_str().unwrap(), r#"{"a":1}"#);
        Ok(())
    }

    #[test]
    fn encode_pretty() -> Result<(), Error> {
        let h = JsonHandler::new(JsonData {
            value: serde_json::json!({"a": 1}),
            indent: JsonIndent::two_spaces(),
            trailing_newline: true,
        });
        let content = h.encode()?;
        let text = content.as_str().unwrap();
        assert!(text.contains("  \"a\""));
        assert!(text.ends_with('\n'));
        Ok(())
    }

    #[tokio::test]
    async fn redact_string_value() -> Result<(), Error> {
        let mut h = compact_handler(r#"{"name":"Alice"}"#);
        let chunk = loop {
            let c = h.next_chunk().await?.expect("expected chunk");
            if c.data.as_str() == "Alice" {
                break c;
            }
        };
        let mut rs = Redactions::new();
        rs.push(chunk.location.clone(), TextReplacement::substituted("Bob"));
        h.redact(rs).await?;
        let encoded = h.encode()?.as_str().unwrap().to_owned();
        assert!(encoded.contains("Bob"));
        assert!(!encoded.contains("Alice"));
        Ok(())
    }
}
