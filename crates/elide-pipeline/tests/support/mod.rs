//! Helpers shared by the pipeline's integration tests.

use elide::modality::Modality;
use elide::{Error, Part, Report, Result};

/// Reading the one document a single-document report holds.
pub trait SoleDocument {
    /// The top-level document, as modality `M`.
    ///
    /// # Errors
    ///
    /// When the report holds no top-level document, or more than
    /// one, or holds it as another modality.
    fn document<M: Modality>(&self) -> Result<Part<'_, M>>;
}

impl SoleDocument for Report {
    fn document<M: Modality>(&self) -> Result<Part<'_, M>> {
        let mut documents = self.part_ids().filter(|(id, _)| id.depth() == 1);
        let (Some((id, _)), None) = (documents.next(), documents.next()) else {
            return Err(Error::malformed("the report holds no sole document"));
        };
        self.part::<M>(id)
    }
}
