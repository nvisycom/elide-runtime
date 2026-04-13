//! JSON handler: holds parsed JSON content and provides span-based
//! access via [`Handler`] + [`TextHandler`].
//!
//! The handler stores the parsed [`serde_json::Value`] tree together
//! with formatting metadata captured during loading, so the original
//! file can be reconstructed with identical whitespace after edits.
//!
//! # Span model
//!
//! The [`TextHandler`] implementation yields string-typed JSON leaves
//! and object keys as text spans addressed by [`TextLocation`]. Byte
//! offsets correspond to positions within the serialized JSON string
//! and are computed via monotonic cursor advancement during tree
//! traversal to avoid ambiguity from duplicate values.
//!
//! # Offset semantics
//!
//! Offsets are into the **serialized** form of the JSON document,
//! including quotes and escape sequences. The `value` field on
//! [`TextLocation`] carries the unescaped string content.

use std::num::NonZeroU32;

use futures::StreamExt;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::{DocumentType, TextFormat};
use nvisy_ontology::entity::TextLocation;
use serde::{Deserialize, Serialize};

use crate::document::{Span, SpanStream};
use crate::handler::text::TextData;
use crate::handler::{Handler, TextHandler};

const DEFAULT_INDENT: NonZeroU32 = NonZeroU32::new(2).unwrap();

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
impl TextHandler for JsonHandler {
    async fn text_spans(&self) -> SpanStream<'_, TextLocation, TextData> {
        let located = self.locate_spans();
        let source = self.source;
        let spans: Vec<_> = located
            .into_iter()
            .map(|ls| {
                Span::new(
                    TextLocation {
                        start_offset: ls.start,
                        end_offset: ls.end,
                        ..Default::default()
                    },
                    TextData::from(ls.text),
                )
                .with_source(source)
            })
            .collect();
        SpanStream::new(futures::stream::iter(spans))
    }

    async fn edit_text(
        &mut self,
        edits: SpanStream<'_, TextLocation, TextData>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        let located = self.locate_spans();

        let mut value_edits = Vec::new();
        let mut key_edits = Vec::new();

        for edit in &edits {
            let ls = located
                .iter()
                .find(|ls| ls.start == edit.id.start_offset && ls.end == edit.id.end_offset)
                .ok_or_else(|| {
                    Error::validation(
                        format!(
                            "no JSON value at byte offset {}..{}",
                            edit.id.start_offset, edit.id.end_offset
                        ),
                        "json-handler",
                    )
                })?;

            if ls.path.key_of {
                key_edits.push((&ls.path, edit.data.clone()));
            } else {
                value_edits.push((&ls.path, edit.data.clone()));
            }
        }

        for (path, data) in &value_edits {
            let target = self.data.value.pointer_mut(&path.pointer).ok_or_else(|| {
                Error::validation(
                    format!("JSON pointer not found: {}", path.pointer),
                    "json-handler",
                )
            })?;
            if target.is_string() {
                *target = serde_json::Value::String(data.clone().into_inner());
            } else {
                let text = data.clone().into_inner();
                *target = serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text));
            }
        }

        for (path, data) in &key_edits {
            rename_key(
                &mut self.data.value,
                &path.pointer,
                &serde_json::Value::String(data.as_str().to_owned()),
            )?;
        }

        Ok(())
    }

    async fn value_at(&self, location: &TextLocation) -> Option<String> {
        // Validate the offset range against known spans to ensure
        // we return a structurally valid value.
        let located = self.locate_spans();
        located
            .iter()
            .find(|ls| ls.start == location.start_offset && ls.end == location.end_offset)
            .map(|ls| ls.text.clone())
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

            // Search for the quoted string or raw value starting from
            // the cursor position. This ensures each match advances
            // monotonically, resolving duplicate-value ambiguity.
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
                .map_err(|e| Error::validation(format!("JSON encode error: {e}"), "json-handler")),
            JsonIndent::Spaces(n) => {
                let indent = " ".repeat(n.get() as usize);
                let mut buf = Vec::new();
                let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
                let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
                serde::Serialize::serialize(&self.data.value, &mut ser).map_err(|e| {
                    Error::validation(format!("JSON encode error: {e}"), "json-handler")
                })?;
                Ok(buf)
            }
            JsonIndent::Tab => {
                let mut buf = Vec::new();
                let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
                let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
                serde::Serialize::serialize(&self.data.value, &mut ser).map_err(|e| {
                    Error::validation(format!("JSON encode error: {e}"), "json-handler")
                })?;
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
fn rename_key(
    root: &mut serde_json::Value,
    pointer: &str,
    new_key_value: &serde_json::Value,
) -> Result<(), Error> {
    let new_key = new_key_value
        .as_str()
        .ok_or_else(|| Error::validation("key rename value must be a string", "json-handler"))?;

    let (parent_ptr, old_key_segment) = pointer.rsplit_once('/').ok_or_else(|| {
        Error::validation(format!("cannot rename root: {pointer}"), "json-handler")
    })?;

    let old_key = old_key_segment.replace("~1", "/").replace("~0", "~");

    let parent = if parent_ptr.is_empty() {
        root
    } else {
        root.pointer_mut(parent_ptr).ok_or_else(|| {
            Error::validation(
                format!("parent pointer not found: {parent_ptr}"),
                "json-handler",
            )
        })?
    };

    let obj = parent
        .as_object_mut()
        .ok_or_else(|| Error::validation("parent is not an object", "json-handler"))?;

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
    use crate::handler::TextHandler;

    fn compact_handler(json: &str) -> JsonHandler {
        JsonHandler::new(JsonData {
            value: serde_json::from_str(json).unwrap(),
            indent: JsonIndent::Compact,
            trailing_newline: false,
        })
    }

    #[tokio::test]
    async fn text_spans_yields_string_leaves() {
        let h = compact_handler(r#"{"name":"Alice","age":30}"#);
        let spans: Vec<_> = h.text_spans().await.collect().await;
        let texts: Vec<_> = spans.iter().map(|s| s.data.as_str()).collect();
        assert!(texts.contains(&"name"));
        assert!(texts.contains(&"Alice"));
        assert!(texts.contains(&"age"));
        assert!(texts.contains(&"30"));
    }

    #[tokio::test]
    async fn duplicate_values_get_distinct_offsets() {
        let h = compact_handler(r#"{"a":"same","b":"same"}"#);
        let spans: Vec<_> = h.text_spans().await.collect().await;
        let same_spans: Vec<_> = spans.iter().filter(|s| s.data.as_str() == "same").collect();
        assert_eq!(same_spans.len(), 2);
        assert_ne!(
            same_spans[0].id.start_offset, same_spans[1].id.start_offset,
            "duplicate values must have distinct offsets"
        );
    }

    #[tokio::test]
    async fn value_at_returns_string() {
        let h = compact_handler(r#"{"name":"Alice"}"#);
        let spans: Vec<_> = h.text_spans().await.collect().await;
        let alice_span = spans.iter().find(|s| s.data.as_str() == "Alice").unwrap();
        assert_eq!(h.value_at(&alice_span.id).await, Some("Alice".to_string()));
    }

    #[tokio::test]
    async fn value_at_rejects_arbitrary_offsets() {
        let h = compact_handler(r#"{"name":"Alice"}"#);
        let bogus = TextLocation {
            start_offset: 3,
            end_offset: 7,
            ..Default::default()
        };
        assert_eq!(h.value_at(&bogus).await, None);
    }

    #[tokio::test]
    async fn nested_structure() {
        let h = compact_handler(r#"{"user":{"name":"Bob","ids":[1,2]}}"#);
        let spans: Vec<_> = h.text_spans().await.collect().await;
        let texts: Vec<_> = spans.iter().map(|s| s.data.as_str()).collect();
        assert!(texts.contains(&"Bob"));
        assert!(texts.contains(&"1"));
        assert!(texts.contains(&"2"));
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
