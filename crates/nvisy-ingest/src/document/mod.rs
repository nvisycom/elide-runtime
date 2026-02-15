//! Unified document representation.

pub mod view_stream;
pub mod edit_stream;

use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;
use nvisy_ontology::entity::DocumentType;

use crate::handler::Handler;

/// A unified representation of any content that can be handled by the pipeline.
///
/// `Document` is generic over `H`, a [`Handler`] that holds the loaded data
/// and provides methods to read and manipulate it.
#[derive(Debug)]
pub struct Document<H: Handler> {
    /// Content source identity and lineage.
    pub source: ContentSource,

    /// Format handler (holds the loaded data).
    handler: H,
}

impl<H: Handler + Clone> Clone for Document<H> {
    fn clone(&self) -> Self {
        Self {
            source: self.source,
            handler: self.handler.clone(),
        }
    }
}

impl<H: Handler> Document<H> {
    /// Create a new document with the given handler.
    pub fn new(handler: H) -> Self {
        Self {
            source: ContentSource::new(),
            handler,
        }
    }

    /// Get a reference to the format handler.
    pub fn handler(&self) -> &H {
        &self.handler
    }

    /// Get a mutable reference to the format handler.
    pub fn handler_mut(&mut self) -> &mut H {
        &mut self.handler
    }

    /// The document type of the loaded content.
    pub fn document_type(&self) -> DocumentType {
        self.handler.document_type()
    }

    /// Set this document's parent to the given content source.
    pub fn with_parent(mut self, content: &ContentData) -> Self {
        self.source.set_parent_id(Some(content.content_source.as_uuid()));
        self
    }
}
