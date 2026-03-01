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
//! and object keys as text spans addressable by [`JsonPath`].  This
//! enables the text redaction pipeline to find and replace PII within
//! JSON string values and keys.
//!
//! For full JSON value access (including non-string leaves), use the
//! inherent [`view_spans`](JsonHandler::view_spans) and
//! [`edit_spans`](JsonHandler::edit_spans) methods that operate on
//! `serde_json::Value` directly.
//!
//! [RFC 6901]: https://www.rfc-editor.org/rfc/rfc6901

use std::num::NonZeroU32;

use futures::StreamExt;
use serde::{Deserialize, Serialize};

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;

use crate::handler::{Handler, Span, SpanEditStream, SpanStream, TextHandler};
use crate::handler::text::TextData;

const DEFAULT_INDENT: NonZeroU32 = NonZeroU32::new(2).unwrap();

/// [RFC 6901] JSON Pointer identifying a span within a JSON document.
///
/// `pointer` follows JSON Pointer syntax: `""` for the root,
/// `"/foo/0/bar"` for nested paths.  Object keys containing `~` or `/`
/// are escaped as `~0` and `~1` respectively.
///
/// When `key_of` is `true` the span addresses the **key name** of the
/// object entry at `pointer`, rather than its value.  Editing a key
/// span renames the key; editing a value span replaces the value.
///
/// [RFC 6901]: https://www.rfc-editor.org/rfc/rfc6901
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JsonPath {
    pub pointer: String,
    pub key_of: bool,
}

impl JsonPath {
    /// Create a value-addressing path.
    pub fn value(pointer: impl Into<String>) -> Self {
        Self {
            pointer: pointer.into(),
            key_of: false,
        }
    }

    /// Create a key-addressing path.
    pub fn key(pointer: impl Into<String>) -> Self {
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
///
/// Provides direct access to the parsed [`serde_json::Value`] tree
/// for reading and mutation, plus [`Handler`] implementation for
/// identity and encoding.
///
/// Implements [`TextHandler`] to expose string-typed leaves and
/// object keys as text spans for the redaction pipeline.
#[derive(Debug)]
pub struct JsonHandler {
    pub(crate) data: JsonData,
}

impl Handler for JsonHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Json
    }

    #[tracing::instrument(name = "json.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<bytes::Bytes, Error> {
        let mut bytes = match self.data.indent {
            JsonIndent::Compact => serde_json::to_vec(&self.data.value)
                .map_err(|e| Error::validation(format!("JSON encode error: {e}"), "json-handler"))?,
            JsonIndent::Spaces(n) => {
                let indent = " ".repeat(n.get() as usize);
                let mut buf = Vec::new();
                let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
                let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
                serde::Serialize::serialize(&self.data.value, &mut ser)
                    .map_err(|e| Error::validation(format!("JSON encode error: {e}"), "json-handler"))?;
                buf
            }
            JsonIndent::Tab => {
                let mut buf = Vec::new();
                let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
                let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
                serde::Serialize::serialize(&self.data.value, &mut ser)
                    .map_err(|e| Error::validation(format!("JSON encode error: {e}"), "json-handler"))?;
                buf
            }
        };
        if self.data.trailing_newline {
            bytes.push(b'\n');
        }
        tracing::Span::current().record("output_bytes", bytes.len());
        Ok(bytes.into())
    }
}

#[async_trait::async_trait]
impl TextHandler for JsonHandler {
    type TextId = JsonPath;

    async fn text_spans(&self) -> SpanStream<'_, JsonPath, TextData> {
        // Yield every leaf value and every object key as TextData.
        // String values yield their string content; non-string leaves
        // yield their JSON serialization; keys yield the key name.
        SpanStream::new(futures::stream::iter(
            JsonSpanIter::new(&self.data.value)
                .map(|span| {
                    let text = match &span.data {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    Span::new(span.id, TextData::from(text)).with_source(span.source)
                })
        ))
    }

    async fn edit_text(
        &mut self,
        edits: SpanEditStream<'_, JsonPath, TextData>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        // Apply value edits first so that pointers remain valid when
        // key renames change the path structure.
        for edit in edits.iter().filter(|e| !e.id.key_of) {
            let target =
                self.data.value.pointer_mut(&edit.id.pointer).ok_or_else(|| {
                    Error::validation(
                        format!("JSON pointer not found: {}", edit.id.pointer),
                        "json-handler",
                    )
                })?;
            // If the original value was a string, replace with the new
            // text directly.  Otherwise, try parsing as JSON and fall
            // back to storing as string.
            if target.is_string() {
                *target = serde_json::Value::String(edit.data.clone().into_inner());
            } else {
                let text = edit.data.clone().into_inner();
                *target = serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text));
            }
        }
        for edit in edits.iter().filter(|e| e.id.key_of) {
            rename_key(
                &mut self.data.value,
                &edit.id.pointer,
                &serde_json::Value::String(edit.data.as_str().to_owned()),
            )?;
        }
        Ok(())
    }
}

impl JsonHandler {
    /// View the JSON tree as an async stream of spans with full
    /// `serde_json::Value` data.
    pub async fn view_spans(&self) -> SpanStream<'_, JsonPath, serde_json::Value> {
        SpanStream::new(futures::stream::iter(JsonSpanIter::new(&self.data.value)))
    }

    /// Apply edits from an async stream to the JSON tree using full
    /// `serde_json::Value` data.
    pub async fn edit_spans(
        &mut self,
        edits: SpanEditStream<'_, JsonPath, serde_json::Value>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        // Apply value edits first so that pointers remain valid when
        // key renames change the path structure.
        for edit in edits.iter().filter(|e| !e.id.key_of) {
            let target =
                self.data.value.pointer_mut(&edit.id.pointer).ok_or_else(|| {
                    Error::validation(
                        format!("JSON pointer not found: {}", edit.id.pointer),
                        "json-handler",
                    )
                })?;
            *target = edit.data.clone();
        }
        for edit in edits.iter().filter(|e| e.id.key_of) {
            rename_key(&mut self.data.value, &edit.id.pointer, &edit.data)?;
        }
        Ok(())
    }

    /// Reference to the root JSON value.
    pub fn value(&self) -> &serde_json::Value {
        &self.data.value
    }

    /// Mutable reference to the root JSON value.
    pub fn value_mut(&mut self) -> &mut serde_json::Value {
        &mut self.data.value
    }

    /// Look up a value by [RFC 6901] JSON Pointer (e.g. `"/a/0/b"`).
    ///
    /// [RFC 6901]: https://www.rfc-editor.org/rfc/rfc6901
    pub fn pointer(&self, pointer: &str) -> Option<&serde_json::Value> {
        self.data.value.pointer(pointer)
    }

    /// Mutably look up a value by [RFC 6901] JSON Pointer.
    ///
    /// [RFC 6901]: https://www.rfc-editor.org/rfc/rfc6901
    pub fn pointer_mut(&mut self, pointer: &str) -> Option<&mut serde_json::Value> {
        self.data.value.pointer_mut(pointer)
    }

    /// Replace the entire root value.
    pub fn set_value(&mut self, value: serde_json::Value) {
        self.data.value = value;
    }

    /// Indentation style detected in the original source.
    pub fn indent(&self) -> JsonIndent {
        self.data.indent
    }

    /// Whether the original source had a trailing newline.
    pub fn trailing_newline(&self) -> bool {
        self.data.trailing_newline
    }

    /// Consume the handler and return the inner [`JsonData`].
    pub fn into_data(self) -> JsonData {
        self.data
    }
}

/// Rename an object key addressed by `pointer`.
///
/// `new_name` must be a `Value::String`; the pointer must resolve to
/// an entry inside an object.
fn rename_key(
    root: &mut serde_json::Value,
    pointer: &str,
    new_name: &serde_json::Value,
) -> Result<(), Error> {
    let new_key = new_name.as_str().ok_or_else(|| {
        Error::validation("key rename requires a string value", "json-handler")
    })?;

    let (parent_ptr, old_key) = split_pointer(pointer)?;

    let parent = if parent_ptr.is_empty() {
        root as &mut serde_json::Value
    } else {
        root.pointer_mut(parent_ptr).ok_or_else(|| {
            Error::validation(
                format!("JSON pointer not found: {parent_ptr}"),
                "json-handler",
            )
        })?
    };

    let obj = parent.as_object_mut().ok_or_else(|| {
        Error::validation(
            format!("parent at {parent_ptr} is not an object"),
            "json-handler",
        )
    })?;

    let value = obj.remove(&old_key).ok_or_else(|| {
        Error::validation(
            format!("key {old_key:?} not found in object at {parent_ptr}"),
            "json-handler",
        )
    })?;

    obj.insert(new_key.to_owned(), value);
    Ok(())
}

/// Split a JSON Pointer into parent pointer and last segment (unescaped).
fn split_pointer(pointer: &str) -> Result<(&str, String), Error> {
    let last_slash = pointer.rfind('/').ok_or_else(|| {
        Error::validation(
            format!("invalid JSON pointer for key rename: {pointer}"),
            "json-handler",
        )
    })?;
    let parent = &pointer[..last_slash];
    let segment = unescape_json_pointer(&pointer[last_slash + 1..]);
    Ok((parent, segment))
}

/// Unescape a JSON Pointer segment ([RFC 6901]): `~1` → `/`, `~0` → `~`.
///
/// [RFC 6901]: https://www.rfc-editor.org/rfc/rfc6901
fn unescape_json_pointer(segment: &str) -> String {
    if segment.contains('~') {
        segment.replace("~1", "/").replace("~0", "~")
    } else {
        segment.to_owned()
    }
}

/// Stack frame for iterative JSON tree traversal.
enum IterFrame<'a> {
    /// A leaf or unexpanded node to process.
    Pending {
        value: &'a serde_json::Value,
        pointer: String,
    },
    /// A key span to yield before descending into its value.
    KeySpan {
        value: &'a serde_json::Value,
        pointer: String,
        key: String,
    },
    /// An object whose entries are being yielded.
    Object(String, serde_json::map::Iter<'a>),
    /// An array whose elements are being yielded.
    Array(String, std::iter::Enumerate<std::slice::Iter<'a, serde_json::Value>>),
}

/// Stack-based depth-first iterator over a JSON tree.
///
/// Yields one [`Span`] per leaf value **and** one per object key.
/// Key spans have [`JsonPath::key_of`] set to `true` and carry the
/// key name as `Value::String`.  Objects and arrays are expanded in
/// place without recursion, so arbitrarily deep documents are safe
/// to iterate.
struct JsonSpanIter<'a> {
    stack: Vec<IterFrame<'a>>,
}

impl<'a> JsonSpanIter<'a> {
    fn new(root: &'a serde_json::Value) -> Self {
        Self {
            stack: vec![IterFrame::Pending {
                value: root,
                pointer: String::new(),
            }],
        }
    }
}

impl<'a> Iterator for JsonSpanIter<'a> {
    type Item = Span<JsonPath, serde_json::Value>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let frame = self.stack.last_mut()?;

            match frame {
                IterFrame::Pending { .. } => {
                    let IterFrame::Pending { value, pointer } =
                        self.stack.pop().unwrap()
                    else {
                        unreachable!()
                    };
                    match value {
                        serde_json::Value::Object(map) => {
                            self.stack.push(IterFrame::Object(pointer, map.iter()));
                        }
                        serde_json::Value::Array(arr) => {
                            self.stack
                                .push(IterFrame::Array(pointer, arr.iter().enumerate()));
                        }
                        leaf => {
                            return Some(Span::new(
                                JsonPath::value(pointer),
                                leaf.clone(),
                            ));
                        }
                    }
                }
                IterFrame::KeySpan { .. } => {
                    let IterFrame::KeySpan { value, pointer, key } =
                        self.stack.pop().unwrap()
                    else {
                        unreachable!()
                    };
                    // Push the value traversal so it runs after we yield the key.
                    self.stack.push(IterFrame::Pending {
                        value,
                        pointer: pointer.clone(),
                    });
                    return Some(Span::new(
                        JsonPath::key(&pointer),
                        serde_json::Value::String(key),
                    ));
                }
                IterFrame::Object(pointer, iter) => match iter.next() {
                    Some((key, child)) => {
                        let child_pointer =
                            format!("{}/{}", pointer, escape_json_pointer(key));
                        self.stack.push(IterFrame::KeySpan {
                            value: child,
                            pointer: child_pointer,
                            key: key.clone(),
                        });
                    }
                    None => {
                        self.stack.pop();
                    }
                },
                IterFrame::Array(pointer, iter) => match iter.next() {
                    Some((i, child)) => {
                        let child_pointer = format!("{}/{i}", pointer);
                        self.stack.push(IterFrame::Pending {
                            value: child,
                            pointer: child_pointer,
                        });
                    }
                    None => {
                        self.stack.pop();
                    }
                },
            }
        }
    }
}

/// Escape a JSON object key for use in a JSON Pointer ([RFC 6901]).
///
/// [RFC 6901]: https://www.rfc-editor.org/rfc/rfc6901
fn escape_json_pointer(key: &str) -> String {
    if key.contains('~') || key.contains('/') {
        key.replace('~', "~0").replace('/', "~1")
    } else {
        key.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::{SpanEdit, TextHandler};
    use futures::StreamExt;
    use nvisy_core::Error;
    use serde_json::json;

    fn handler(value: serde_json::Value) -> JsonHandler {
        JsonHandler {
            data: JsonData {
                value,
                ..JsonData::default()
            },
        }
    }

    #[tokio::test]
    async fn text_spans_flat_object() {
        let h = handler(json!({"name": "Alice", "age": 30}));
        let spans: Vec<_> = h.text_spans().await.collect().await;

        // BTreeMap (alphabetical): age before name.
        // Each key emits a key span followed by a value span.
        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0].id, JsonPath::key("/age"));
        assert_eq!(spans[0].data, "age");
        assert_eq!(spans[1].id, JsonPath::value("/age"));
        assert_eq!(spans[1].data, "30"); // non-string leaf serialized
        assert_eq!(spans[2].id, JsonPath::key("/name"));
        assert_eq!(spans[2].data, "name");
        assert_eq!(spans[3].id, JsonPath::value("/name"));
        assert_eq!(spans[3].data, "Alice"); // string leaf
    }

    #[tokio::test]
    async fn view_spans_flat_object() {
        let h = handler(json!({"name": "Alice", "age": 30}));
        let spans: Vec<_> = h.view_spans().await.collect().await;

        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0].id, JsonPath::key("/age"));
        assert_eq!(spans[0].data, json!("age"));
        assert_eq!(spans[1].id, JsonPath::value("/age"));
        assert_eq!(spans[1].data, json!(30));
        assert_eq!(spans[2].id, JsonPath::key("/name"));
        assert_eq!(spans[2].data, json!("name"));
        assert_eq!(spans[3].id, JsonPath::value("/name"));
        assert_eq!(spans[3].data, json!("Alice"));
    }

    #[tokio::test]
    async fn view_spans_nested() {
        let h = handler(json!({"a": {"b": [1, "two", null]}}));
        let spans: Vec<_> = h.view_spans().await.collect().await;

        assert_eq!(spans.len(), 5);
        assert_eq!(spans[0].id, JsonPath::key("/a"));
        assert_eq!(spans[1].id, JsonPath::key("/a/b"));
        assert_eq!(spans[2].id, JsonPath::value("/a/b/0"));
        assert_eq!(spans[2].data, json!(1));
        assert_eq!(spans[3].id, JsonPath::value("/a/b/1"));
        assert_eq!(spans[3].data, json!("two"));
        assert_eq!(spans[4].id, JsonPath::value("/a/b/2"));
        assert_eq!(spans[4].data, json!(null));
    }

    #[tokio::test]
    async fn view_spans_key_escaping() {
        let h = handler(json!({"a/b": "x", "c~d": "y"}));
        let spans: Vec<_> = h.view_spans().await.collect().await;

        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0].id, JsonPath::key("/a~1b"));
        assert_eq!(spans[0].data, json!("a/b"));
        assert_eq!(spans[1].id, JsonPath::value("/a~1b"));
        assert_eq!(spans[1].data, json!("x"));
        assert_eq!(spans[2].id, JsonPath::key("/c~0d"));
        assert_eq!(spans[2].data, json!("c~d"));
        assert_eq!(spans[3].id, JsonPath::value("/c~0d"));
        assert_eq!(spans[3].data, json!("y"));
    }

    #[tokio::test]
    async fn edit_text_replace_string_value() -> Result<(), Error> {
        let mut h = handler(json!({"ssn": "123-45-6789"}));
        h.edit_text(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit::new(JsonPath::value("/ssn"), "[REDACTED]".into()),
        ])))
        .await?;
        assert_eq!(h.value(), &json!({"ssn": "[REDACTED]"}));
        Ok(())
    }

    #[tokio::test]
    async fn edit_spans_replace_value() -> Result<(), Error> {
        let mut h = handler(json!({"ssn": "123-45-6789"}));
        h.edit_spans(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit::new(JsonPath::value("/ssn"), json!(null)),
        ])))
        .await?;
        assert_eq!(h.value(), &json!({"ssn": null}));
        Ok(())
    }

    #[tokio::test]
    async fn edit_spans_rename_key() -> Result<(), Error> {
        let mut h = handler(json!({"John Smith": {"age": 30}}));
        h.edit_spans(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit::new(JsonPath::key("/John Smith"), json!("[REDACTED]")),
        ])))
        .await?;
        assert_eq!(h.value(), &json!({"[REDACTED]": {"age": 30}}));
        Ok(())
    }

    #[tokio::test]
    async fn edit_text_rename_key() -> Result<(), Error> {
        let mut h = handler(json!({"John Smith": {"age": 30}}));
        h.edit_text(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit::new(JsonPath::key("/John Smith"), "[REDACTED]".into()),
        ])))
        .await?;
        assert_eq!(h.value(), &json!({"[REDACTED]": {"age": 30}}));
        Ok(())
    }

    #[tokio::test]
    async fn edit_spans_rename_nested_key() -> Result<(), Error> {
        let mut h = handler(json!({"a": {"secret_field": 42}}));
        h.edit_spans(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit::new(JsonPath::key("/a/secret_field"), json!("redacted")),
        ])))
        .await?;
        assert_eq!(h.value(), &json!({"a": {"redacted": 42}}));
        Ok(())
    }

    #[tokio::test]
    async fn edit_spans_rename_key_requires_string() {
        let mut h = handler(json!({"a": 1}));
        let err = h
            .edit_spans(SpanEditStream::new(futures::stream::iter(vec![
                SpanEdit::new(JsonPath::key("/a"), json!(42)),
            ])))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("string"));
    }

    #[tokio::test]
    async fn edit_spans_bad_pointer() {
        let mut h = handler(json!({"a": 1}));
        let err = h
            .edit_spans(SpanEditStream::new(futures::stream::iter(vec![
                SpanEdit::new(JsonPath::value("/nonexistent"), json!(null)),
            ])))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn edit_spans_value_before_key_rename() -> Result<(), Error> {
        let mut h = handler(json!({"name": "Alice"}));
        h.edit_spans(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit::new(JsonPath::key("/name"), json!("[REDACTED]")),
            SpanEdit::new(JsonPath::value("/name"), json!("***")),
        ])))
        .await?;
        assert_eq!(h.value(), &json!({"[REDACTED]": "***"}));
        Ok(())
    }

    #[test]
    fn encode_compact() -> Result<(), Error> {
        let h = JsonHandler {
            data: JsonData {
                value: json!({"a": 1}),
                indent: JsonIndent::Compact,
                trailing_newline: false,
            },
        };
        let bytes = h.encode()?;
        assert_eq!(std::str::from_utf8(&bytes).expect("valid utf-8"), r#"{"a":1}"#);
        Ok(())
    }

    #[test]
    fn encode_two_spaces_with_trailing_newline() -> Result<(), Error> {
        let h = JsonHandler {
            data: JsonData {
                value: json!({"a": 1}),
                indent: JsonIndent::two_spaces(),
                trailing_newline: true,
            },
        };
        let text = std::str::from_utf8(&h.encode()?).expect("valid utf-8").to_owned();
        assert!(text.contains("  \"a\""));
        assert!(text.ends_with('\n'));
        Ok(())
    }

    #[test]
    fn encode_tab_indent() -> Result<(), Error> {
        let h = JsonHandler {
            data: JsonData {
                value: json!({"a": 1}),
                indent: JsonIndent::Tab,
                trailing_newline: false,
            },
        };
        let text = std::str::from_utf8(&h.encode()?).expect("valid utf-8").to_owned();
        assert!(text.contains("\t\"a\""));
        assert!(!text.ends_with('\n'));
        Ok(())
    }
}
