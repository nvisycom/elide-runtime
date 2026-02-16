//! JSON handler — holds parsed JSON content and provides span-based
//! access via [`Handler`].
//!
//! The handler stores the parsed [`serde_json::Value`] tree together
//! with formatting metadata captured during loading, so the original
//! file can be reconstructed with identical whitespace after edits.
//!
//! # Span model
//!
//! [`Handler::view_spans`] yields one [`Span`] per node in the JSON
//! tree.  **Every** value is emitted — leaf scalars, and object keys
//! (as string-valued spans).  Each span is addressed by a [`JsonPath`]:
//! an [RFC 6901] JSON Pointer such as `/address/city` plus a flag
//! indicating whether the span targets the key name or the value.
//!
//! [`Handler::edit_spans`] accepts [`SpanEdit`]s.  For value spans the
//! value at the pointer is replaced; for key spans the object key is
//! renamed.
//!
//! [RFC 6901]: https://www.rfc-editor.org/rfc/rfc6901

use std::num::NonZeroU32;

use futures::StreamExt;
use serde::{Deserialize, Serialize};

use nvisy_core::error::Error;
use nvisy_ontology::entity::DocumentType;

use crate::document::edit_stream::SpanEditStream;
use crate::document::view_stream::SpanStream;
use crate::handler::{Handler, Span};

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
/// pipeline-driven span-based editing.
#[derive(Debug)]
pub struct JsonHandler {
    pub(crate) data: JsonData,
}

#[async_trait::async_trait]
impl Handler for JsonHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Json
    }

    type SpanId = JsonPath;
    type SpanData = serde_json::Value;

    async fn view_spans(&self) -> SpanStream<'_, JsonPath, serde_json::Value> {
        SpanStream::new(futures::stream::iter(JsonSpanIter::new(&self.data.value)))
    }

    async fn edit_spans(
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
}

impl JsonHandler {
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
                            return Some(Span {
                                id: JsonPath::value(pointer),
                                data: leaf.clone(),
                            });
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
                    return Some(Span {
                        id: JsonPath::key(&pointer),
                        data: serde_json::Value::String(key),
                    });
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
    use crate::handler::SpanEdit;
    use futures::StreamExt;
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
    async fn view_spans_flat_object() {
        let h = handler(json!({"name": "Alice", "age": 30}));
        let spans: Vec<_> = h.view_spans().await.collect().await;

        // BTreeMap (alphabetical): age before name.
        // Each key emits a key span followed by a value span.
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

        // key "a", key "b", values 0/1/2
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

        // key span, value span, key span, value span
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
    async fn view_spans_empty_object() {
        let h = handler(json!({}));
        let spans: Vec<_> = h.view_spans().await.collect().await;
        assert!(spans.is_empty());
    }

    #[tokio::test]
    async fn view_spans_scalar_root() {
        let h = handler(json!("hello"));
        let spans: Vec<_> = h.view_spans().await.collect().await;
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].id, JsonPath::value(""));
        assert_eq!(spans[0].data, json!("hello"));
    }

    #[tokio::test]
    async fn edit_spans_replace_value() {
        let mut h = handler(json!({"ssn": "123-45-6789"}));
        h.edit_spans(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit {
                id: JsonPath::value("/ssn"),
                data: json!(null),
            },
        ])))
        .await
        .unwrap();
        assert_eq!(h.value(), &json!({"ssn": null}));
    }

    #[tokio::test]
    async fn edit_spans_rename_key() {
        let mut h = handler(json!({"John Smith": {"age": 30}}));
        h.edit_spans(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit {
                id: JsonPath::key("/John Smith"),
                data: json!("[REDACTED]"),
            },
        ])))
        .await
        .unwrap();
        assert_eq!(h.value(), &json!({"[REDACTED]": {"age": 30}}));
    }

    #[tokio::test]
    async fn edit_spans_rename_nested_key() {
        let mut h = handler(json!({"a": {"secret_field": 42}}));
        h.edit_spans(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit {
                id: JsonPath::key("/a/secret_field"),
                data: json!("redacted"),
            },
        ])))
        .await
        .unwrap();
        assert_eq!(h.value(), &json!({"a": {"redacted": 42}}));
    }

    #[tokio::test]
    async fn edit_spans_rename_key_requires_string() {
        let mut h = handler(json!({"a": 1}));
        let err = h
            .edit_spans(SpanEditStream::new(futures::stream::iter(vec![
                SpanEdit {
                    id: JsonPath::key("/a"),
                    data: json!(42),
                },
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
                SpanEdit {
                    id: JsonPath::value("/nonexistent"),
                    data: json!(null),
                },
            ])))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn edit_spans_value_before_key_rename() {
        let mut h = handler(json!({"name": "Alice"}));
        // Key rename listed first, but value edit must apply first
        // (while /name still exists) before the key is renamed.
        h.edit_spans(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit {
                id: JsonPath::key("/name"),
                data: json!("[REDACTED]"),
            },
            SpanEdit {
                id: JsonPath::value("/name"),
                data: json!("***"),
            },
        ])))
        .await
        .unwrap();
        assert_eq!(h.value(), &json!({"[REDACTED]": "***"}));
    }
}
