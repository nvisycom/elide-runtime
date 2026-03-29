//! Semaphore construction for [`ConcurrencyPolicy`].

use std::sync::Arc;

use nvisy_ontology::workflow::ConcurrencyPolicy;
use tokio::sync::Semaphore;

/// Converts a [`ConcurrencyPolicy`] into a tokio semaphore.
pub(crate) trait ConcurrencyExt {
    /// Create a semaphore with the configured number of permits.
    fn to_semaphore(&self) -> Arc<Semaphore>;
}

impl ConcurrencyExt for ConcurrencyPolicy {
    fn to_semaphore(&self) -> Arc<Semaphore> {
        Arc::new(Semaphore::new(self.max_nodes))
    }
}
