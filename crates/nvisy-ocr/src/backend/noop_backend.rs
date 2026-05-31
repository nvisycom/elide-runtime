//! [`NoopBackend`] — returns no OCR results. Default selection
//! for deployments where the runtime should accept image content
//! but isn't expected to recognise text in it.
//!
//! Useful in tests, as a placeholder while wiring up a real
//! externalised backend, and for redaction pipelines that operate
//! purely on metadata or on entities sourced from elsewhere.

use nvisy_core::Error;
use nvisy_ontology::document::Block;
use nvisy_ontology::entity::{ModelKind, ModelProvenance};
use nvisy_ontology::modality::Image;

use crate::core::{Backend, Context, ImageInput};

/// A [`Backend`] that produces no OCR results.
///
/// Every call returns an empty block list.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopBackend;

impl NoopBackend {
    /// Construct an empty backend.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Backend for NoopBackend {
    fn provenance(&self) -> ModelProvenance {
        ModelProvenance::new("noop-ocr", ModelKind::SelfHosted)
    }

    async fn run(
        &self,
        _image: &ImageInput,
        _ctx: Context<'_>,
    ) -> Result<Vec<Block<Image>>, Error> {
        Ok(Vec::new())
    }
}
