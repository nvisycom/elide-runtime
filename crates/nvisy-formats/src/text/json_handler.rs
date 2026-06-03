//! JSON handler: holds parsed JSON content and provides location-based
//! access via [`Handler`] + [`TextHandler`].
//!
//! The handler stores the parsed [`Value`] tree together
//! with formatting metadata captured during loading, so the original
//! file can be reconstructed with identical whitespace after edits.
//!
//! [`TextHandler::locations`] yields string-typed JSON leaves and
//! object keys, addressed by [`Text`]. Byte offsets correspond
//! to positions within the serialized JSON string and are computed via
//! monotonic cursor advancement during tree traversal to avoid
//! ambiguity from duplicate values.
//!
//! Offsets are into the **serialized** form (including quotes and
//! escapes). [`TextHandler::read`] returns the unescaped string value
//! at a location.
//!
//! [`Value`]: serde_json::Value

use std::num::NonZeroU32;

use nvisy_codec::core::{Handle, Located, LocationStream};
use nvisy_codec::handler::{Handler, TextData, TextRedaction};
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource, DocumentType, TextFormat};
use nvisy_core::modality::{Text, TextLocation};
use serde::{Deserialize, Serialize};

use super::redact;

const DEFAULT_INDENT: NonZeroU32 = NonZeroU32::new(2).unwrap();
const TARGET: &str = "json-handler";

/// [RFC 6901] JSON Pointer identifying a span within a JSON document.
///
/// Used internally by `JsonHandler` for tree navigation.
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

/// Handler for loaded JSON content.
#[derive(Debug)]
pub struct JsonHandler {
    source: ContentSource,
    data: JsonData,
}

/// A located JSON span with its path and byte offset range.
struct LocatedSpan {
    path: JsonPath,
    text: String,
    start: usize,
    end: usize,
}

#[async_trait::async_trait]
impl Handle<Text> for JsonHandler {
    fn locations(&self) -> LocationStream<'_, TextLocation> {
        let source = self.source;
        let items: Vec<_> = self
            .locate_spans()
            .into_iter()
            .map(|ls| {
                Located::new(
                    source,
                    TextLocation {
                        start: ls.start,
                        end: ls.end,
                        ..Default::default()
                    },
                )
            })
            .collect();
        LocationStream::new(futures::stream::iter(items))
    }

    async fn read(&self, location: &TextLocation) -> Option<TextData> {
        self.locate_spans()
            .into_iter()
            .find(|ls| ls.start == location.start && ls.end == location.end)
            .map(|ls| TextData::from(ls.text))
    }

    async fn redact_at(
        &mut self,
        location: &TextLocation,
        redaction: TextRedaction,
    ) -> Result<(), Error> {
        let located = self.locate_spans();
        let Some(ls) = located
            .into_iter()
            .find(|ls| location.start >= ls.start && location.end <= ls.end)
        else {
            return Ok(());
        };
        let start = location.start - ls.start;
        let end = location.end - ls.start;
        let mut content = ls.text.clone();
        let value = redaction.output().replacement_value().unwrap_or_default();
        redact::replace_range(&mut content, value, start, end, TARGET)?;
        if ls.path.key_of {
            rename_key(&mut self.data.value, &ls.path.pointer, &content)?;
        } else {
            let target = self
                .data
                .value
                .pointer_mut(&ls.path.pointer)
                .ok_or_else(|| {
                    Error::validation(
                        format!("JSON pointer not found: {}", ls.path.pointer),
                        TARGET,
                    )
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
}

impl Handler for JsonHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Text(TextFormat::Json)
    }

    fn source(&self) -> ContentSource {
        self.source
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

impl JsonHandler {
    /// Create a new handler from parsed JSON data.
    pub fn new(data: JsonData) -> Self {
        Self {
            source: ContentSource::new(),
            data,
        }
    }

    /// Set the content source for lineage tracking.
    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }

    /// Access the underlying JSON value.
    pub fn value(&self) -> &serde_json::Value {
        &self.data.value
    }

    /// Mutable access to the underlying JSON value.
    pub fn value_mut(&mut self) -> &mut serde_json::Value {
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

    /// Compute located spans by serializing and tracking byte positions
    /// with a monotonic cursor to avoid duplicate-value ambiguity.
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
    use futures::StreamExt;
    use nvisy_core::Error;

    use super::*;

    fn compact_handler(json: &str) -> JsonHandler {
        JsonHandler::new(JsonData {
            value: serde_json::from_str(json).unwrap(),
            indent: JsonIndent::Compact,
            trailing_newline: false,
        })
    }

    #[tokio::test]
    async fn locations_string_leaves() {
        let h = compact_handler(r#"{"name":"Alice","age":30}"#);
        let items: Vec<_> = h.locations().collect().await;
        // 2 keys + 2 leaves
        assert_eq!(items.len(), 4);
    }

    #[tokio::test]
    async fn duplicate_values_get_distinct_offsets() {
        let h = compact_handler(r#"{"a":"same","b":"same"}"#);
        let mut same_offsets = Vec::new();
        for item in h.locations().collect::<Vec<_>>().await {
            if let Some(td) = h.read(&item.location).await
                && td.as_str() == "same"
            {
                same_offsets.push(item.location.start);
            }
        }
        assert_eq!(same_offsets.len(), 2);
        assert_ne!(same_offsets[0], same_offsets[1]);
    }

    #[tokio::test]
    async fn read_returns_string() {
        let h = compact_handler(r#"{"name":"Alice"}"#);
        let items: Vec<_> = h.locations().collect().await;
        let alice = futures::future::join_all(items.iter().map(|l| h.read(&l.location))).await;
        assert!(
            alice
                .iter()
                .any(|d| d.as_ref().map(|d| d.as_str()) == Some("Alice"))
        );
    }

    #[tokio::test]
    async fn read_rejects_arbitrary_offsets() {
        let h = compact_handler(r#"{"name":"Alice"}"#);
        let bogus = TextLocation {
            start: 3,
            end: 7,
            ..Default::default()
        };
        assert!(h.read(&bogus).await.is_none());
    }

    #[tokio::test]
    async fn nested_structure() {
        let h = compact_handler(r#"{"user":{"name":"Bob","ids":[1,2]}}"#);
        let items: Vec<_> = h.locations().collect().await;
        let mut reads = Vec::new();
        for it in &items {
            if let Some(td) = h.read(&it.location).await {
                reads.push(td.as_str().to_owned());
            }
        }
        assert!(reads.iter().any(|s| s == "Bob"));
        assert!(reads.iter().any(|s| s == "1"));
        assert!(reads.iter().any(|s| s == "2"));
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
}
