//! [`NodeHandler`] trait: envelope-in, envelope-out transform.

use nvisy_core::Error;

use crate::operation::DocumentEnvelope;

/// A node handler transforms one [`DocumentEnvelope`] at a time.
///
/// Each implementation wraps one or more [`Operation`]s, extracting
/// the right data from the envelope, calling the operation, and
/// applying the result via [`ApplyPatch`].
///
/// [`Operation`]: crate::operation::Operation
/// [`ApplyPatch`]: crate::operation::envelope::ApplyPatch
#[async_trait::async_trait]
pub(crate) trait NodeHandler: Send + Sync {
    /// Transform a single envelope, returning the (possibly modified) envelope.
    async fn handle(&self, envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error>;
}
