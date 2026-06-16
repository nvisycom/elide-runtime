//! HTML loader: parses raw HTML into an [`HtmlHandler`] populated
//! with a heterogeneous [`RedactableItem`] stream.
//!
//! Text nodes, every element attribute, and HTML comments are
//! emitted as items. Attribute values pass through verbatim;
//! percent-encoded URL values are a known gap, tracked at
//! <https://github.com/nvisycom/runtime/issues/267>. `<script>`
//! and `<style>` bodies follow [`HtmlLoader::script_policy`] /
//! [`HtmlLoader::style_policy`].
//!
//! [`RedactableItem`]: super::RedactableItem

use ego_tree::NodeRef;
use nvisy_core::Error;
use nvisy_core::modality::Text;
use scraper::Html;
use scraper::node::Node;

use super::html_handler::{ElementTarget, HtmlData, HtmlHandler, RedactableItem, RedactableKind};
use crate::Loader;
use crate::content::{ContentData, ContentSource, TextEncoding};

const TARGET: &str = "nvisy_codec::handler::text::html";

/// Loader for HTML files. Produces one [`HtmlHandler`] per input.
#[derive(Debug, Clone)]
pub struct HtmlLoader {
    /// Character encoding of the input bytes.
    pub encoding: TextEncoding,
    /// How `<script>` element bodies enter the detection stream.
    pub script_policy: ScriptPolicy,
    /// How `<style>` element bodies enter the detection stream.
    pub style_policy: ScriptPolicy,
}

impl Default for HtmlLoader {
    fn default() -> Self {
        Self {
            encoding: TextEncoding::Utf8,
            script_policy: ScriptPolicy::Skip,
            style_policy: ScriptPolicy::Skip,
        }
    }
}

/// How the loader handles `<script>` or `<style>` element bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptPolicy {
    /// Skip the element entirely — its body never enters the
    /// detection stream.
    Skip,
    /// Treat the element body as plain text and scan it like a
    /// regular text node.
    ScanText,
    /// Parse the element body as JSON and route it through the
    /// JSON handler's slot model.
    ///
    /// **Not yet implemented** — a loader configured with this
    /// policy returns an error at decode time. Tracked in
    /// <https://github.com/nvisycom/runtime/issues/266>.
    ScanJson,
}

#[async_trait::async_trait]
impl Loader<Text> for HtmlLoader {
    type Handler = HtmlHandler;

    #[tracing::instrument(name = "html.decode", skip_all, fields(input_bytes, items))]
    async fn decode(&self, content: ContentData) -> Result<HtmlHandler, Error> {
        if matches!(self.script_policy, ScriptPolicy::ScanJson)
            || matches!(self.style_policy, ScriptPolicy::ScanJson)
        {
            return Err(Error::validation(
                "ScriptPolicy::ScanJson is not yet implemented; see https://github.com/nvisycom/runtime/issues/266",
                TARGET,
            ));
        }

        let parent = content.content_source;
        let raw = content.to_bytes();
        tracing::Span::current().record("input_bytes", raw.len());
        let text = self.encoding.decode_bytes(&raw, TARGET)?;
        let dom = Html::parse_document(&text);
        let items = build_items(&dom, self);
        tracing::Span::current().record("items", items.len());

        let source = ContentSource::new().with_parent(&parent);
        Ok(HtmlHandler::new(HtmlData { items, raw: text }).with_source(source))
    }
}

fn build_items(dom: &Html, loader: &HtmlLoader) -> Vec<RedactableItem> {
    let mut items = Vec::new();
    let mut text_index: usize = 0;
    let mut comment_index: usize = 0;
    let mut element_index: usize = 0;

    for node in dom.tree.nodes() {
        match node.value() {
            Node::Text(t) => {
                if !skip_text_under(node) {
                    let hints = sibling_text_hint(node, &t.text);
                    items.push(RedactableItem {
                        kind: RedactableKind::TextNode { index: text_index },
                        value: t.text.to_string(),
                        hints,
                    });
                }
                text_index += 1;
            }
            Node::Comment(c) => {
                items.push(RedactableItem {
                    kind: RedactableKind::Comment {
                        index: comment_index,
                    },
                    value: c.comment.to_string(),
                    hints: Vec::new(),
                });
                comment_index += 1;
            }
            Node::Element(e) => {
                let element_name = e.name.local.as_ref();

                // Every attribute on this element. Values pass
                // through verbatim — URLs like `mailto:alice@x.com`
                // have the email matched in place by the
                // recognizer.
                for (qn, val) in &e.attrs {
                    items.push(RedactableItem {
                        kind: RedactableKind::Element {
                            element_index,
                            target: ElementTarget::Attribute {
                                attr_name: qn.local.as_ref().to_owned(),
                            },
                        },
                        value: val.to_string(),
                        hints: Vec::new(),
                    });
                }

                // `<script>` / `<style>` body, when policy says ScanText.
                let policy = match element_name {
                    "script" => Some(loader.script_policy),
                    "style" => Some(loader.style_policy),
                    _ => None,
                };
                if let Some(ScriptPolicy::ScanText) = policy
                    && let Some(body) = first_child_text(node)
                {
                    let target = if element_name == "script" {
                        ElementTarget::ScriptText
                    } else {
                        ElementTarget::StyleText
                    };
                    items.push(RedactableItem {
                        kind: RedactableKind::Element {
                            element_index,
                            target,
                        },
                        value: body,
                        hints: Vec::new(),
                    });
                }

                element_index += 1;
            }
            _ => {}
        }
    }

    items
}

/// Collect the surrounding-text content of the text node's
/// nearest block-level ancestor as a single hint string.
///
/// Used by [`build_items`] to surface the surrounding sentence
/// (`"the payment card 4111… is on file"`) as an out-of-band
/// hint when a text node sits inside an inline wrapper
/// (`<code>4111…</code>`) that splits the prose into multiple
/// chunks. The walk targets the nearest *block* ancestor
/// (`<p>`, `<div>`, `<li>`, `<td>`, `<th>`, `<h1>`–`<h6>`,
/// `<blockquote>`, `<dt>`, `<dd>`) — stopping at the immediate
/// inline parent would yield only the chunk's own text.
///
/// `own_text` is excluded so the hint doesn't echo the node's
/// own bytes. Returns an empty `Vec` when no useful surrounding
/// text exists (no block ancestor, or the ancestor contains
/// only this text).
fn sibling_text_hint(text_node: NodeRef<'_, Node>, own_text: &str) -> Vec<String> {
    let Some(ancestor) = nearest_block_ancestor(text_node) else {
        return Vec::new();
    };
    let mut buf = String::new();
    for descendant in ancestor.descendants() {
        if let Node::Text(t) = descendant.value() {
            let chunk = t.text.as_ref();
            if chunk == own_text {
                continue;
            }
            if !buf.is_empty() {
                buf.push(' ');
            }
            buf.push_str(chunk);
        }
    }
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![trimmed.to_owned()]
    }
}

/// Walk parents until we hit a block-level element (or root).
/// Used to find the "sentence boundary" around an inline text
/// node so the hint covers the full prose around an inline
/// wrapper like `<code>` or `<span>`.
fn nearest_block_ancestor(text_node: NodeRef<'_, Node>) -> Option<NodeRef<'_, Node>> {
    let mut current = text_node.parent();
    while let Some(node) = current {
        if let Node::Element(e) = node.value() {
            if is_block_element(e.name.local.as_ref()) {
                return Some(node);
            }
        }
        current = node.parent();
    }
    None
}

fn is_block_element(name: &str) -> bool {
    matches!(
        name,
        "p" | "div"
            | "li"
            | "td"
            | "th"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "blockquote"
            | "dt"
            | "dd"
            | "section"
            | "article"
            | "aside"
            | "header"
            | "footer"
            | "main"
            | "nav"
            | "figcaption"
            | "caption"
    )
}

/// Don't emit text-node items for text that lives directly inside
/// a `<script>` or `<style>` element — those bodies are handled by
/// the script / style policy on the parent element instead. The
/// `text_index` counter still advances so encode's
/// document-order index lines up with decode.
fn skip_text_under(text_node: NodeRef<'_, Node>) -> bool {
    text_node
        .parent()
        .and_then(|p| p.value().as_element())
        .map(|e| {
            let name = e.name.local.as_ref();
            matches!(name, "script" | "style")
        })
        .unwrap_or(false)
}

fn first_child_text(node: NodeRef<'_, Node>) -> Option<String> {
    let child = node.first_child()?;
    match child.value() {
        Node::Text(t) => Some(t.text.to_string()),
        _ => None,
    }
}
