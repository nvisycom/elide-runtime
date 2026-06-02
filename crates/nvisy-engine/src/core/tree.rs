//! [`DocumentTree`]: the per-source carrier every phase mutates.
//!
//! Replaces the old `DocumentEnvelope<M>` + `AnyEnvelope` pair. A
//! `DocumentTree` holds:
//!
//! - a modality-erased root [`AnyDocument`] (one of Text / Tabular /
//!   Image / Audio),
//! - the shared codec [`SharedHandle`] backing the underlying bytes
//!   for source reads and redaction writes,
//! - the originating [`ContentMetadata`] policy evaluation reads
//!   against.
//!
//! Rich sources (PDF/DOCX) materialise as one `AnyDocument::Text`
//! whose blocks host nested image/tabular documents via
//! [`TextBlock::Embed`]. The whole pipeline operates on the *tree*,
//! not on per-modality envelopes — phases visit [`DocumentTree::root_mut`]
//! first and then iterate [`DocumentTree::embeds_mut`].
//!
//! [`TextBlock::Embed`]: nvisy_ontology::modality::TextBlock::Embed

use std::slice::IterMut;

use nvisy_core::content::ContentMetadata;
use nvisy_ontology::document::{Block, Document};
use nvisy_ontology::modality::{Audio, EmbeddedDocument, Image, Tabular, Text, TextBlock};
use nvisy_ontology::provenance::AnyAudit;

use super::SharedHandle;

/// Per-source unit of work for the pipeline. One [`DocumentTree`]
/// per imported file; each phase mutates it in place.
pub struct DocumentTree {
    /// Modality-erased root document. Rich sources surface as
    /// [`AnyDocument::Text`] with nested image/tabular docs under
    /// [`TextBlock::Embed`] children.
    ///
    /// [`TextBlock::Embed`]: nvisy_ontology::modality::TextBlock::Embed
    pub root: AnyDocument,

    /// Shared codec handle for source reads and redaction writes.
    /// `Arc<Mutex<_>>` because handle redaction methods take
    /// `&mut self`; phases coordinate access through the lock.
    pub handle: SharedHandle,

    /// Content metadata (MIME type, filename, …) from the original
    /// upload. Policy evaluation matches rules against fields here;
    /// detection forwards it to recognizers that need source
    /// context.
    pub metadata: ContentMetadata,
}

/// Modality-erased root document for a [`DocumentTree`].
///
/// One variant per modality. Rich sources always land in
/// [`AnyDocument::Text`]; their image/tabular content lives inside
/// the text doc as nested [`Document<Image>`] / [`Document<Tabular>`]
/// under [`TextBlock::Embed`] children — never as separate
/// [`AnyDocument`] roots.
///
/// [`TextBlock::Embed`]: nvisy_ontology::modality::TextBlock::Embed
#[derive(Debug)]
#[non_exhaustive]
pub enum AnyDocument {
    Text(Document<Text>),
    Tabular(Document<Tabular>),
    Image(Document<Image>),
    Audio(Document<Audio>),
}

/// Mutable borrow of a single node in a [`DocumentTree`] walk. The
/// variant tag selects the modality; each arm carries the
/// `&mut Document<M>` for that modality.
///
/// Returned by [`DocumentTree::root_mut`] and yielded by
/// [`DocumentTree::embeds_mut`]; phases visit the root first, then
/// iterate any nested embedded docs in block order.
pub enum NodeMut<'a> {
    Text(&'a mut Document<Text>),
    Tabular(&'a mut Document<Tabular>),
    Image(&'a mut Document<Image>),
    Audio(&'a mut Document<Audio>),
}

impl NodeMut<'_> {
    /// Stable lowercase name of the node's modality. Used for
    /// per-node tracing spans.
    pub fn modality_name(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Tabular(_) => "tabular",
            Self::Image(_) => "image",
            Self::Audio(_) => "audio",
        }
    }
}

impl AnyDocument {
    /// Human-readable name of the modality. Used for telemetry and
    /// error messages.
    pub fn modality_name(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Tabular(_) => "tabular",
            Self::Image(_) => "image",
            Self::Audio(_) => "audio",
        }
    }
}

impl DocumentTree {
    /// Construct a new tree from its parts.
    pub fn new(root: AnyDocument, handle: SharedHandle, metadata: ContentMetadata) -> Self {
        Self {
            root,
            handle,
            metadata,
        }
    }

    /// Clone the root document's audit as a modality-erased
    /// [`AnyAudit`]. Used by the run summary collector so it doesn't
    /// have to match on every variant.
    pub fn audit_cloned(&self) -> AnyAudit {
        match &self.root {
            AnyDocument::Text(doc) => doc.audit.clone().into(),
            AnyDocument::Tabular(doc) => doc.audit.clone().into(),
            AnyDocument::Image(doc) => doc.audit.clone().into(),
            AnyDocument::Audio(doc) => doc.audit.clone().into(),
        }
    }

    /// Mutable borrow of the root document as a typed [`NodeMut`].
    ///
    /// Phases visit the root first (one borrow scope), then iterate
    /// nested embeds via [`Self::embeds_mut`] (a fresh borrow scope).
    /// Splitting the walk this way avoids the closure-lifetime
    /// problem that would arise from a single `walk_mut(|node| …)`
    /// callback: each phase's per-node async body is free to borrow
    /// from the surrounding scope without HRTB / `Send` gymnastics.
    pub fn root_mut(&mut self) -> NodeMut<'_> {
        match &mut self.root {
            AnyDocument::Text(doc) => NodeMut::Text(doc),
            AnyDocument::Tabular(doc) => NodeMut::Tabular(doc),
            AnyDocument::Image(doc) => NodeMut::Image(doc),
            AnyDocument::Audio(doc) => NodeMut::Audio(doc),
        }
    }

    /// Mutable iterator over the root's nested embedded documents,
    /// in block order. Empty for non-Text roots and for Text roots
    /// with no [`TextBlock::Embed`] blocks.
    ///
    /// Sync rather than `async` because the recursion structure is
    /// deterministic — only the per-node work the caller does is
    /// async. The returned iterator type is intentionally generic so
    /// callers can chain with `.enumerate()`, `.try_for_each(…)`,
    /// etc., and so the borrow checker can see each yielded
    /// `NodeMut<'_>` as a fresh exclusive borrow.
    ///
    /// [`TextBlock::Embed`]: nvisy_ontology::modality::TextBlock::Embed
    pub fn embeds_mut(&mut self) -> EmbedsMut<'_> {
        let blocks = match &mut self.root {
            AnyDocument::Text(doc) => Some(doc.blocks.iter_mut()),
            AnyDocument::Tabular(_) | AnyDocument::Image(_) | AnyDocument::Audio(_) => None,
        };
        EmbedsMut { blocks }
    }
}

/// Mutable embed-walker returned by [`DocumentTree::embeds_mut`].
///
/// Yields one [`NodeMut`] per [`TextBlock::Embed`] block in source
/// order; non-embed blocks are skipped. The walker holds the only
/// `&mut` borrow of the root's blocks for its lifetime, so phases
/// must finish the iteration before re-borrowing the tree.
///
/// [`TextBlock::Embed`]: nvisy_ontology::modality::TextBlock::Embed
pub struct EmbedsMut<'a> {
    blocks: Option<IterMut<'a, Block<Text>>>,
}

impl<'a> Iterator for EmbedsMut<'a> {
    type Item = NodeMut<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let blocks = self.blocks.as_mut()?;
        for block in blocks.by_ref() {
            let TextBlock::Embed(embed) = &mut block.kind else {
                continue;
            };
            return Some(match embed.as_mut() {
                EmbeddedDocument::Image(nested) => NodeMut::Image(nested),
                EmbeddedDocument::Tabular(nested) => NodeMut::Tabular(nested),
            });
        }
        None
    }
}
