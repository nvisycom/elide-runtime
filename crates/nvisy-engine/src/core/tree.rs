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
//! not on per-modality envelopes — phases walk it with
//! [`DocumentTree::walk_mut`] and dispatch per node.
//!
//! [`TextBlock::Embed`]: nvisy_ontology::modality::TextBlock::Embed

use nvisy_core::Result;
use nvisy_core::content::ContentMetadata;
use nvisy_ontology::document::Document;
use nvisy_ontology::modality::{Audio, EmbeddedDocument, Image, Tabular, Text, TextBlock};

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
/// Yielded by [`DocumentTree::walk_mut`] in pre-order traversal:
/// the root is visited first, then any nested embedded docs in
/// block order.
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

    /// Visit every node in the tree in pre-order (root first, then
    /// each nested embed in block order). The closure is invoked
    /// once per node with a typed [`NodeMut`].
    ///
    /// Pre-order: a Text root with two image embeds gets visited
    /// as `[Text, Image, Image]`. The Text body sees the doc *with*
    /// embed placeholders in `blocks`; the embed bodies see only
    /// their nested doc.
    ///
    /// Closure may `.await`. Errors short-circuit the walk.
    pub async fn walk_mut<F, Fut>(&mut self, mut visit: F) -> Result<()>
    where
        F: FnMut(NodeMut<'_>) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        match &mut self.root {
            AnyDocument::Text(doc) => {
                visit(NodeMut::Text(doc)).await?;
                // Walk nested embeds. The root borrow above
                // released when its `visit` future resolved, so
                // re-borrowing `doc.blocks` here is fine.
                for block in doc.blocks.iter_mut() {
                    let TextBlock::Embed(embed) = &mut block.kind else {
                        continue;
                    };
                    match embed.as_mut() {
                        EmbeddedDocument::Image(nested) => {
                            visit(NodeMut::Image(nested)).await?;
                        }
                        EmbeddedDocument::Tabular(nested) => {
                            visit(NodeMut::Tabular(nested)).await?;
                        }
                    }
                }
            }
            AnyDocument::Tabular(doc) => visit(NodeMut::Tabular(doc)).await?,
            AnyDocument::Image(doc) => visit(NodeMut::Image(doc)).await?,
            AnyDocument::Audio(doc) => visit(NodeMut::Audio(doc)).await?,
        }
        Ok(())
    }
}
