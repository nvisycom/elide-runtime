//! Compute replacement values and apply redaction records to
//! document content via codec transforms.
//!
//! [`RedactionApplicator`] is currently a no-op stub — the per-
//! modality apply machinery is being reworked in the same pass that
//! re-introduces policy evaluation against the new [`Strategy`]
//! enum. The shape is preserved so the engine pipeline still wires
//! through end-to-end while the implementation is reinstated.
//!
//! [`Strategy`]: nvisy_ontology::policy::Strategy

use nvisy_core::Result;
use nvisy_ontology::modality::Text;

use crate::envelope::DocumentEnvelope;

/// Per-envelope redaction applicator (currently a no-op).
pub(super) struct RedactionApplicator<'a> {
    envelope: &'a mut DocumentEnvelope<Text>,
}

impl<'a> RedactionApplicator<'a> {
    pub fn new(envelope: &'a mut DocumentEnvelope<Text>) -> Self {
        Self { envelope }
    }

    pub async fn apply(self) -> Result<()> {
        // Suppress the unused-field lint on `envelope` without losing
        // the reference: the hook stays in place so wiring is visible.
        let _ = self.envelope;
        Ok(())
    }
}
