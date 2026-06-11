//! HTML loader: validates and parses raw HTML into an
//! [`HtmlHandler`] populated with a heterogeneous
//! [`RedactableItem`] stream.
//!
//! What gets included in the stream is controlled by the policy
//! fields on [`HtmlLoader`] — attributes ([`scan_attributes`]),
//! comments ([`scan_comments`]), URL bodies inside scheme-known
//! attributes ([`scan_url_schemes`]), and `<script>` / `<style>`
//! element bodies ([`script_policy`] / [`style_policy`]).
//!
//! [`scan_attributes`]: HtmlLoader::scan_attributes
//! [`scan_comments`]: HtmlLoader::scan_comments
//! [`scan_url_schemes`]: HtmlLoader::scan_url_schemes
//! [`script_policy`]: HtmlLoader::script_policy
//! [`style_policy`]: HtmlLoader::style_policy
//! [`RedactableItem`]: super::RedactableItem

use std::borrow::Cow;

use nvisy_core::Error;
use nvisy_core::modality::Text;
use scraper::Html;

use super::html_handler::{ElementTarget, HtmlData, HtmlHandler, RedactableItem, RedactableKind};
use crate::Loader;
use crate::content::{ContentData, ContentSource, TextEncoding};

const TARGET: &str = "nvisy_codec::handler::text::html";

/// Loader for HTML files. Produces one [`HtmlHandler`] per input.
///
/// Every policy field has a default that enables a reasonable
/// scanning surface for typical web pages; set fields explicitly to
/// narrow (or widen) what gets fed through detection.
#[derive(Debug, Clone)]
pub struct HtmlLoader {
    /// Character encoding of the input bytes. Defaults to UTF-8.
    pub encoding: TextEncoding,
    /// Attribute local names whose values get scanned. Entries
    /// ending with `*` are treated as prefix wildcards (so
    /// `"data-*"` matches every `data-` attribute). Default:
    /// [`default_scan_attributes`].
    pub scan_attributes: Vec<Cow<'static, str>>,
    /// Whether HTML comment bodies get scanned. Default: `true` —
    /// comments are a known PII leak vector (debug notes that ship
    /// to production).
    pub scan_comments: bool,
    /// URL schemes whose body (the text after `scheme:`) gets
    /// scanned when the loader sees one of those URLs in `href` or
    /// `src`. Default: every [`UrlScheme`] variant.
    pub scan_url_schemes: Vec<UrlScheme>,
    /// What to do with `<script>` element bodies. Default:
    /// [`ScriptPolicy::Skip`] — script bodies are usually code and
    /// scanning them for PII produces false positives.
    pub script_policy: ScriptPolicy,
    /// What to do with `<style>` element bodies. Default:
    /// [`ScriptPolicy::Skip`] — CSS rarely carries PII.
    pub style_policy: ScriptPolicy,
}

impl Default for HtmlLoader {
    fn default() -> Self {
        Self {
            encoding: TextEncoding::Utf8,
            scan_attributes: default_scan_attributes(),
            scan_comments: true,
            scan_url_schemes: vec![
                UrlScheme::Mailto,
                UrlScheme::Tel,
                UrlScheme::Sms,
                UrlScheme::Callto,
            ],
            script_policy: ScriptPolicy::Skip,
            style_policy: ScriptPolicy::Skip,
        }
    }
}

/// Default attribute scan list: text-bearing attributes that
/// commonly carry user data.
///
/// - `alt` — image text alternatives, often readable sentences.
/// - `title` — tooltip text shown on hover.
/// - `value` — form input values; can be pre-seeded with PII.
/// - `placeholder` — form hints.
/// - `aria-label` / `aria-describedby` — accessibility text
///   vocalised by screen readers.
/// - `data-*` — application-specific data attributes; common
///   dumping ground for hydration payloads.
#[must_use]
pub fn default_scan_attributes() -> Vec<Cow<'static, str>> {
    vec![
        Cow::Borrowed("alt"),
        Cow::Borrowed("title"),
        Cow::Borrowed("value"),
        Cow::Borrowed("placeholder"),
        Cow::Borrowed("aria-label"),
        Cow::Borrowed("aria-describedby"),
        Cow::Borrowed("data-*"),
    ]
}

/// URL schemes whose body gets scanned when present in `href` or
/// `src` attributes. Each variant matches a single RFC-defined
/// scheme prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UrlScheme {
    /// `mailto:user@host(?query)` — RFC 6068. Body is everything
    /// after `mailto:` up to the first `?` or `#`.
    Mailto,
    /// `tel:+1-415-555-0142` — RFC 3966. Body is everything after
    /// `tel:` up to the first `?` or `#`.
    Tel,
    /// `sms:+1-415-555-0142` — RFC 5724. Same shape as `tel:`.
    Sms,
    /// `callto:+1-415-555-0142` — Skype legacy. Same shape as
    /// `tel:`.
    Callto,
}

impl UrlScheme {
    /// The `scheme:` prefix encode reattaches when reassembling the
    /// URL value after a redact.
    #[must_use]
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Mailto => "mailto:",
            Self::Tel => "tel:",
            Self::Sms => "sms:",
            Self::Callto => "callto:",
        }
    }

    fn parse(value: &str) -> Option<(Self, &str, &str)> {
        for scheme in [Self::Mailto, Self::Tel, Self::Sms, Self::Callto] {
            if let Some(rest) = value.strip_prefix(scheme.prefix()) {
                let split_at = rest.find(['?', '#']).unwrap_or(rest.len());
                let (body, suffix) = rest.split_at(split_at);
                return Some((scheme, body, suffix));
            }
        }
        None
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
            scraper::node::Node::Text(t) => {
                if !skip_text_under(node) {
                    items.push(RedactableItem {
                        kind: RedactableKind::TextNode { index: text_index },
                        value: t.text.to_string(),
                    });
                }
                text_index += 1;
            }
            scraper::node::Node::Comment(c) => {
                if loader.scan_comments {
                    items.push(RedactableItem {
                        kind: RedactableKind::Comment {
                            index: comment_index,
                        },
                        value: c.comment.to_string(),
                    });
                }
                comment_index += 1;
            }
            scraper::node::Node::Element(e) => {
                let element_name = e.name.local.as_ref();

                // Attribute-bound items (Attribute + UrlBody) at this element.
                for (qn, val) in &e.attrs {
                    let attr_name = qn.local.as_ref();
                    let is_url_attr =
                        matches!(attr_name, "href" | "src") && !loader.scan_url_schemes.is_empty();
                    if is_url_attr
                        && let Some((scheme, body, suffix)) = UrlScheme::parse(val.as_ref())
                        && loader.scan_url_schemes.contains(&scheme)
                    {
                        items.push(RedactableItem {
                            kind: RedactableKind::Element {
                                element_index,
                                target: ElementTarget::UrlBody {
                                    attr_name: attr_name.to_owned(),
                                    scheme,
                                    suffix: suffix.to_owned(),
                                },
                            },
                            value: body.to_owned(),
                        });
                        continue;
                    }
                    if matches_scan_attribute(&loader.scan_attributes, attr_name) {
                        items.push(RedactableItem {
                            kind: RedactableKind::Element {
                                element_index,
                                target: ElementTarget::Attribute {
                                    attr_name: attr_name.to_owned(),
                                },
                            },
                            value: val.to_string(),
                        });
                    }
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
                    });
                }

                element_index += 1;
            }
            _ => {}
        }
    }

    items
}

/// Match an attribute name against the configured scan list,
/// honouring `*`-suffixed prefix wildcards (e.g. `data-*` matches
/// every attribute starting with `data-`).
fn matches_scan_attribute(scan_list: &[Cow<'static, str>], name: &str) -> bool {
    for entry in scan_list {
        if let Some(prefix) = entry.strip_suffix('*') {
            if name.starts_with(prefix) {
                return true;
            }
        } else if entry.as_ref() == name {
            return true;
        }
    }
    false
}

/// Don't emit text-node items for text that lives directly inside
/// a `<script>` or `<style>` element — those bodies are handled by
/// the script / style policy on the parent element instead. The
/// `text_index` counter still advances so encode's
/// document-order index lines up with decode.
fn skip_text_under(text_node: ego_tree::NodeRef<'_, scraper::node::Node>) -> bool {
    text_node
        .parent()
        .and_then(|p| p.value().as_element())
        .map(|e| {
            let name = e.name.local.as_ref();
            matches!(name, "script" | "style")
        })
        .unwrap_or(false)
}

fn first_child_text(node: ego_tree::NodeRef<'_, scraper::node::Node>) -> Option<String> {
    let child = node.first_child()?;
    match child.value() {
        scraper::node::Node::Text(t) => Some(t.text.to_string()),
        _ => None,
    }
}
