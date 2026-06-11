//! HTML handler: parses HTML into a heterogeneous stream of
//! [`RedactableItem`]s — text nodes, comments, element attributes,
//! URL bodies inside scheme-known attributes, and `<script>` /
//! `<style>` bodies. Each item participates in the same
//! `decode → chunk → detect → redact → encode` flow.
//!
//! Offsets are cumulative over the redactable-item sequence in
//! document order, not raw HTML source bytes. [`Handler::encode`]
//! reconstructs the HTML by re-parsing the original source into a
//! DOM, replaying each item's mutation in the same order the
//! loader visited it, and serializing back with [`Html::html`].
//!
//! What gets scanned is controlled at decode time via the
//! [`HtmlLoader`] policy fields ([`scan_attributes`],
//! [`scan_comments`], [`scan_url_schemes`], [`script_policy`],
//! [`style_policy`]).
//!
//! [`Html::html`]: scraper::Html::html
//! [`HtmlLoader`]: super::HtmlLoader
//! [`scan_attributes`]: super::HtmlLoader::scan_attributes
//! [`scan_comments`]: super::HtmlLoader::scan_comments
//! [`scan_url_schemes`]: super::HtmlLoader::scan_url_schemes
//! [`script_policy`]: super::HtmlLoader::script_policy
//! [`style_policy`]: super::HtmlLoader::style_policy

use std::ops::Range;

use nvisy_core::Error;
use nvisy_core::modality::{Text, TextData, TextLocation};
use nvisy_core::redaction::{Redactions, TextReplacement};

use super::{HtmlLoader, UrlScheme, lift_identity, redact};
use crate::content::{ContentData, ContentSource};
use crate::{Chunk, Format, FormatId, Handler};

const TARGET: &str = "nvisy_codec::handler::text::html";

/// Stable [`FormatId`] for the HTML codec.
pub const FORMAT_ID: FormatId = FormatId::from_static("nvisy.text.html");

/// [`Format`] descriptor registered into [`crate::CodecRegistry`].
pub fn format() -> Format {
    Format::new::<Text, _>(FORMAT_ID.clone(), HtmlLoader::default())
        .with_extensions(["html", "htm"])
        .with_content_types(["text/html"])
}

/// Parsed HTML content: the redactable-item stream plus the raw
/// source bytes (kept for round-trip reconstruction).
#[derive(Debug, Clone)]
pub struct HtmlData {
    /// Redactable items in document order. Decoded once by the
    /// [`HtmlLoader`] and never re-derived afterwards.
    pub items: Vec<RedactableItem>,
    /// The raw HTML source. Re-parsed at encode time so the
    /// serialiser can splice mutated values back into a fresh
    /// DOM and emit the result.
    ///
    /// [`HtmlLoader::decode`]: super::HtmlLoader
    pub raw: String,
}

/// One redactable unit yielded by [`HtmlHandler::next_chunk`].
///
/// `value` is the decoded text the recognizer scans and that
/// [`HtmlHandler::redact`] mutates in place. `kind` tells encode
/// where to splice the mutated value back into the document.
#[derive(Debug, Clone)]
pub struct RedactableItem {
    /// Where this item lives in the document.
    pub kind: RedactableKind,
    /// The decoded value: text-node text, comment body, attribute
    /// value, URL body (without the scheme prefix), or script /
    /// style element text.
    pub value: String,
}

/// Where a [`RedactableItem`] lives inside the parsed HTML
/// document. Used by encode to splice the mutated `value` back into
/// a freshly-parsed DOM.
#[derive(Debug, Clone)]
pub enum RedactableKind {
    /// A text node, addressed by its 0-based index in the
    /// document-order text-node sequence.
    TextNode {
        /// Document-order index among text nodes.
        index: usize,
    },
    /// An HTML comment, addressed by its 0-based index in the
    /// document-order comment sequence.
    Comment {
        /// Document-order index among comments.
        index: usize,
    },
    /// Any element-bound item: an attribute value, a URL body
    /// inside a scheme-known attribute, or a script / style
    /// element's text body.
    Element {
        /// Document-order index among elements.
        element_index: usize,
        /// Which part of the element this item addresses.
        target: ElementTarget,
    },
}

/// The element-bound location a [`RedactableKind::Element`] item
/// points at.
#[derive(Debug, Clone)]
pub enum ElementTarget {
    /// The value of `attr_name` on this element.
    Attribute {
        /// Attribute local name.
        attr_name: String,
    },
    /// The body of a scheme-known URL stored in `attr_name`
    /// (typically `href` or `src`). The item's `value` is the
    /// scannable body without the `scheme:` prefix and any
    /// trailing query string; encode re-attaches them.
    UrlBody {
        /// Attribute local name carrying the URL (`href` / `src`).
        attr_name: String,
        /// URL scheme recognised at decode time.
        scheme: UrlScheme,
        /// Optional query / fragment suffix that decode peeled off
        /// (everything from the first `?` or `#` onwards).
        /// Re-attached verbatim at encode.
        suffix: String,
    },
    /// The text body of a `<script>` element scanned as plain text.
    ScriptText,
    /// The text body of a `<style>` element scanned as plain text.
    StyleText,
}

/// Handler for loaded HTML content.
///
/// `item_starts` is a cumulative-offset index over the redactable
/// items: `item_starts[i]` is the byte position of item `i`, and
/// `item_starts[items.len()]` is the total length sentinel.
/// Maintained on every redaction so random-access reads run in
/// `O(log N)`.
#[derive(Debug)]
pub struct HtmlHandler {
    source: ContentSource,
    data: HtmlData,
    item_starts: Vec<usize>,
    cursor: usize,
}

#[async_trait::async_trait]
impl Handler<Text> for HtmlHandler {
    fn format(&self) -> FormatId {
        FORMAT_ID.clone()
    }

    fn source(&self) -> ContentSource {
        self.source
    }

    #[tracing::instrument(name = "html.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<ContentData, Error> {
        let mut dom = scraper::Html::parse_document(&self.data.raw);
        let plan = EncodePlan::from_items(&self.data.items);

        let mut text_seen = 0usize;
        let mut comment_seen = 0usize;
        let mut element_seen = 0usize;

        let node_ids: Vec<_> = dom.tree.nodes().map(|n| n.id()).collect();
        for node_id in node_ids {
            let Some(node) = dom.tree.get(node_id) else {
                continue;
            };
            match node.value() {
                scraper::node::Node::Text(_) => {
                    if let Some(value) = plan.text_value(text_seen) {
                        write_text(&mut dom, node_id, value);
                    }
                    text_seen += 1;
                }
                scraper::node::Node::Comment(_) => {
                    if let Some(value) = plan.comment_value(comment_seen) {
                        write_comment(&mut dom, node_id, value);
                    }
                    comment_seen += 1;
                }
                scraper::node::Node::Element(_) => {
                    if let Some(targets) = plan.element_targets(element_seen) {
                        for et in targets {
                            apply_element_target(&mut dom, node_id, et);
                        }
                    }
                    element_seen += 1;
                }
                _ => {}
            }
        }

        let bytes = dom.html().into_bytes();
        tracing::Span::current().record("output_bytes", bytes.len());
        let source = ContentSource::new().with_parent(&self.source);
        Ok(ContentData::new(source, bytes.into()))
    }

    async fn next_chunk(&mut self) -> Result<Option<Chunk<Text>>, Error> {
        if self.cursor >= self.data.items.len() {
            return Ok(None);
        }
        let i = self.cursor;
        let start = self.item_starts[i];
        let end = self.item_starts[i + 1];
        let value = &self.data.items[i].value;
        self.cursor += 1;
        Ok(Some(Chunk {
            location: TextLocation {
                start,
                end,
                ..Default::default()
            },
            data: TextData::from(value.as_str()),
        }))
    }

    fn lift_chunk(&self, chunk: &Chunk<Text>, value_range: Range<usize>) -> Option<TextLocation> {
        lift_identity(chunk, value_range)
    }

    async fn read(&self, location: &TextLocation) -> Result<Option<TextData>, Error> {
        let Some(i) = self.item_for(location.start) else {
            return Ok(None);
        };
        let item_start = self.item_starts[i];
        let item_end = self.item_starts[i + 1];
        if location.end > item_end {
            return Ok(None);
        }
        let local_start = location.start - item_start;
        let local_end = location.end - item_start;
        Ok(self.data.items[i]
            .value
            .get(local_start..local_end)
            .map(TextData::from))
    }

    async fn redact(&mut self, mut redactions: Redactions<Text>) -> Result<(), Error> {
        redactions.sort_descending();
        for (location, replacement) in redactions.into_items() {
            self.redact_one(&location, replacement)?;
        }
        Ok(())
    }
}

impl HtmlHandler {
    /// Build a handler from a populated [`HtmlData`].
    pub fn new(data: HtmlData) -> Self {
        let item_starts = compute_item_starts(&data.items);
        Self {
            source: ContentSource::new(),
            data,
            item_starts,
            cursor: 0,
        }
    }

    /// Attach a content source for lineage tracking.
    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }

    /// All redactable items in document order.
    pub fn items(&self) -> &[RedactableItem] {
        &self.data.items
    }

    /// Total number of redactable items.
    pub fn len(&self) -> usize {
        self.data.items.len()
    }

    /// Whether the document has no redactable items.
    pub fn is_empty(&self) -> bool {
        self.data.items.is_empty()
    }

    /// The raw HTML source.
    pub fn raw(&self) -> &str {
        &self.data.raw
    }

    /// Consume the handler and return the inner [`HtmlData`].
    pub fn into_data(self) -> HtmlData {
        self.data
    }

    /// Rewind the streaming cursor to the start of the document.
    pub fn rewind(&mut self) {
        self.cursor = 0;
    }

    fn item_for(&self, byte_offset: usize) -> Option<usize> {
        match self.item_starts.binary_search(&byte_offset) {
            Ok(i) if i < self.data.items.len() => Some(i),
            Ok(_) => None,
            Err(i) if i > 0 && i <= self.data.items.len() => Some(i - 1),
            _ => None,
        }
    }

    fn shift_starts_after(&mut self, i: usize, delta: isize) {
        if delta == 0 {
            return;
        }
        for s in &mut self.item_starts[i + 1..] {
            *s = (*s as isize + delta) as usize;
        }
    }

    fn redact_one(
        &mut self,
        location: &TextLocation,
        replacement: TextReplacement,
    ) -> Result<(), Error> {
        let Some(i) = self.item_for(location.start) else {
            return Ok(());
        };
        let item_start = self.item_starts[i];
        let item_end = self.item_starts[i + 1];
        if location.end > item_end {
            return Ok(());
        }
        let local_start = location.start - item_start;
        let local_end = location.end - item_start;
        let value = replacement.replacement_value().unwrap_or_default();
        let before_len = self.data.items[i].value.len();
        redact::replace_range(
            &mut self.data.items[i].value,
            value,
            local_start,
            local_end,
            TARGET,
        )?;
        let delta = self.data.items[i].value.len() as isize - before_len as isize;
        self.shift_starts_after(i, delta);
        Ok(())
    }
}

/// Cumulative byte-offset table over the redactable items: `[0, len(item[0]),
/// len(item[0]) + len(item[1]), …, total]`.
fn compute_item_starts(items: &[RedactableItem]) -> Vec<usize> {
    let mut starts = Vec::with_capacity(items.len() + 1);
    let mut offset = 0usize;
    for item in items {
        starts.push(offset);
        offset += item.value.len();
    }
    starts.push(offset);
    starts
}

/// Pre-bucketed view of the loader's items, indexed by kind so
/// encode's single DOM walk can answer "what does item-of-kind-X at
/// index N look like?" in O(1).
struct EncodePlan<'a> {
    /// Indexed by text-node ordinal in document order. `Some(value)`
    /// when the loader emitted an item for that text node;
    /// `None` for skipped text nodes (e.g. script/style children
    /// when the policy is Skip).
    text_values: Vec<Option<&'a str>>,
    /// Indexed by comment ordinal. Same shape as `text_values`.
    comment_values: Vec<Option<&'a str>>,
    /// Indexed by element ordinal. Each entry is the (possibly
    /// empty) list of element-bound items at that element.
    element_targets: Vec<Vec<ElementTargetPatch<'a>>>,
}

/// A single element-bound patch staged for encode.
struct ElementTargetPatch<'a> {
    target: &'a ElementTarget,
    value: &'a str,
}

impl<'a> EncodePlan<'a> {
    fn from_items(items: &'a [RedactableItem]) -> Self {
        let mut text_values: Vec<Option<&'a str>> = Vec::new();
        let mut comment_values: Vec<Option<&'a str>> = Vec::new();
        let mut element_targets: Vec<Vec<ElementTargetPatch<'a>>> = Vec::new();

        for item in items {
            match &item.kind {
                RedactableKind::TextNode { index } => {
                    grow_to(&mut text_values, *index);
                    text_values[*index] = Some(item.value.as_str());
                }
                RedactableKind::Comment { index } => {
                    grow_to(&mut comment_values, *index);
                    comment_values[*index] = Some(item.value.as_str());
                }
                RedactableKind::Element {
                    element_index,
                    target,
                } => {
                    while element_targets.len() <= *element_index {
                        element_targets.push(Vec::new());
                    }
                    element_targets[*element_index].push(ElementTargetPatch {
                        target,
                        value: item.value.as_str(),
                    });
                }
            }
        }

        Self {
            text_values,
            comment_values,
            element_targets,
        }
    }

    fn text_value(&self, ordinal: usize) -> Option<&'a str> {
        self.text_values.get(ordinal).copied().flatten()
    }

    fn comment_value(&self, ordinal: usize) -> Option<&'a str> {
        self.comment_values.get(ordinal).copied().flatten()
    }

    fn element_targets(&self, ordinal: usize) -> Option<&[ElementTargetPatch<'a>]> {
        self.element_targets.get(ordinal).map(|v| v.as_slice())
    }
}

fn grow_to(v: &mut Vec<Option<&str>>, index: usize) {
    while v.len() <= index {
        v.push(None);
    }
}

fn write_text(dom: &mut scraper::Html, node_id: ego_tree::NodeId, value: &str) {
    if let Some(mut n) = dom.tree.get_mut(node_id)
        && let scraper::node::Node::Text(t) = n.value()
        && t.text.as_ref() != value
    {
        t.text = value.into();
    }
}

fn write_comment(dom: &mut scraper::Html, node_id: ego_tree::NodeId, value: &str) {
    if let Some(mut n) = dom.tree.get_mut(node_id)
        && let scraper::node::Node::Comment(c) = n.value()
        && c.comment.as_ref() != value
    {
        c.comment = value.into();
    }
}

fn apply_element_target(
    dom: &mut scraper::Html,
    node_id: ego_tree::NodeId,
    patch: &ElementTargetPatch<'_>,
) {
    match patch.target {
        ElementTarget::Attribute { attr_name } => {
            write_attribute(dom, node_id, attr_name, patch.value);
        }
        ElementTarget::UrlBody {
            attr_name,
            scheme,
            suffix,
        } => {
            let rebuilt = format!("{}{}{}", scheme.prefix(), patch.value, suffix);
            write_attribute(dom, node_id, attr_name, &rebuilt);
        }
        ElementTarget::ScriptText | ElementTarget::StyleText => {
            write_text_child(dom, node_id, patch.value);
        }
    }
}

fn write_attribute(
    dom: &mut scraper::Html,
    element_id: ego_tree::NodeId,
    attr_name: &str,
    value: &str,
) {
    if let Some(mut n) = dom.tree.get_mut(element_id)
        && let scraper::node::Node::Element(e) = n.value()
    {
        for (qn, val) in &mut e.attrs {
            if qn.local.as_ref() == attr_name {
                if val.as_ref() != value {
                    *val = value.into();
                }
                return;
            }
        }
    }
}

fn write_text_child(dom: &mut scraper::Html, element_id: ego_tree::NodeId, value: &str) {
    let child_id = dom
        .tree
        .get(element_id)
        .and_then(|n| n.first_child().map(|c| c.id()));
    let Some(child_id) = child_id else {
        return;
    };
    write_text(dom, child_id, value);
}

#[cfg(test)]
mod tests {
    use nvisy_core::Error;
    use nvisy_core::redaction::TextReplacement;

    use super::super::HtmlLoader;
    use super::*;
    use crate::Loader;
    use crate::content::{ContentData, ContentSource};

    async fn load(raw: &str) -> HtmlHandler {
        load_with(raw, HtmlLoader::default()).await
    }

    async fn load_with(raw: &str, loader: HtmlLoader) -> HtmlHandler {
        let content = ContentData::new(ContentSource::new(), bytes::Bytes::from(raw.to_owned()));
        loader.decode(content).await.expect("html decode succeeds")
    }

    #[tokio::test]
    async fn encode_unchanged_round_trips() -> Result<(), Error> {
        let raw = "<html><head></head><body><p>Hello</p></body></html>";
        let h = load(raw).await;
        let content = h.encode()?;
        assert_eq!(content.as_str().unwrap(), raw);
        Ok(())
    }

    #[tokio::test]
    async fn redact_replaces_text_node() -> Result<(), Error> {
        let raw = "<html><head></head><body><p>Hello</p><p>World</p></body></html>";
        let mut h = load(raw).await;
        let first = h.next_chunk().await?.unwrap();
        let mut rs = Redactions::new();
        rs.push(first.location, TextReplacement::substituted("[REDACTED]"));
        h.redact(rs).await?;
        let out = h.encode()?.as_str().unwrap().to_owned();
        assert!(out.contains("[REDACTED]"));
        assert!(out.contains("World"));
        Ok(())
    }

    #[tokio::test]
    async fn stream_yields_text_attribute_and_comment() -> Result<(), Error> {
        let raw = r#"<html><head></head><body><!-- secret 1 --><img alt="hello" title="alt"></body></html>"#;
        let mut h = load(raw).await;
        let mut values: Vec<String> = Vec::new();
        while let Some(chunk) = h.next_chunk().await? {
            values.push(chunk.data.as_str().to_owned());
        }
        assert!(values.iter().any(|v| v == " secret 1 "));
        assert!(values.iter().any(|v| v == "hello"));
        assert!(values.iter().any(|v| v == "alt"));
        Ok(())
    }

    #[tokio::test]
    async fn attribute_redact_round_trips() -> Result<(), Error> {
        let raw = r#"<html><head></head><body><img alt="alice@example.com"></body></html>"#;
        let mut h = load(raw).await;
        // Find the attribute chunk.
        let mut loc = None;
        while let Some(chunk) = h.next_chunk().await? {
            if chunk.data.as_str() == "alice@example.com" {
                loc = Some(chunk.location);
                break;
            }
        }
        let loc = loc.expect("alt attribute yields a chunk");
        let mut rs = Redactions::new();
        rs.push(loc, TextReplacement::substituted("[email]"));
        h.redact(rs).await?;
        let out = h.encode()?.as_str().unwrap().to_owned();
        assert!(
            out.contains(r#"alt="[email]""#),
            "alt attribute not rewritten: {out}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn comment_redact_round_trips() -> Result<(), Error> {
        let raw = "<html><head></head><body><!-- alice@example.com --></body></html>";
        let mut h = load(raw).await;
        let mut loc = None;
        while let Some(chunk) = h.next_chunk().await? {
            if chunk.data.as_str().contains("alice@example.com") {
                loc = Some(TextLocation {
                    start: chunk.location.start + chunk.data.as_str().find("alice").unwrap(),
                    end: chunk.location.start
                        + chunk.data.as_str().find("alice").unwrap()
                        + "alice@example.com".len(),
                    ..Default::default()
                });
                break;
            }
        }
        let loc = loc.expect("comment yields a chunk containing the email");
        let mut rs = Redactions::new();
        rs.push(loc, TextReplacement::substituted("[email]"));
        h.redact(rs).await?;
        let out = h.encode()?.as_str().unwrap().to_owned();
        assert!(
            out.contains("<!-- [email] -->"),
            "comment not rewritten: {out}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn url_body_redact_reattaches_scheme() -> Result<(), Error> {
        let raw = r#"<html><head></head><body><a href="mailto:alice@example.com?subject=Hi">contact</a></body></html>"#;
        let mut h = load(raw).await;
        let mut loc = None;
        while let Some(chunk) = h.next_chunk().await? {
            if chunk.data.as_str() == "alice@example.com" {
                loc = Some(chunk.location);
                break;
            }
        }
        let loc = loc.expect("mailto body yields a chunk");
        let mut rs = Redactions::new();
        rs.push(loc, TextReplacement::substituted("[email]"));
        h.redact(rs).await?;
        let out = h.encode()?.as_str().unwrap().to_owned();
        assert!(
            out.contains(r#"href="mailto:[email]?subject=Hi""#),
            "mailto url not rewritten with scheme + suffix: {out}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn opt_out_of_attribute_scanning() -> Result<(), Error> {
        let raw = r#"<html><head></head><body><img alt="alice@example.com"></body></html>"#;
        let loader = HtmlLoader {
            scan_attributes: Vec::new(),
            ..HtmlLoader::default()
        };
        let mut h = load_with(raw, loader).await;
        let mut emitted = Vec::new();
        while let Some(chunk) = h.next_chunk().await? {
            emitted.push(chunk.data.as_str().to_owned());
        }
        assert!(
            emitted.iter().all(|v| v != "alice@example.com"),
            "alt attribute should not have been scanned when opt-out empties the list: {emitted:?}"
        );
        Ok(())
    }
}
