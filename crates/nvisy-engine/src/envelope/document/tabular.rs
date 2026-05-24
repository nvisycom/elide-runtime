//! Tabular (spreadsheet) accessors for [`Document`].

use futures::StreamExt;
use nvisy_codec::Located;
use nvisy_codec::handler::{Redactions, TabularRedaction, TextData};
use nvisy_core::Error;
use nvisy_ontology::entity::TabularLocation;

use super::Document;

impl Document {
    /// Collect all tabular (cell) locations into a `Vec`.
    pub async fn collect_tabular_locations(&self) -> Vec<Located<TabularLocation>> {
        self.handle.tabular_locations().collect().await
    }

    /// Read the cell value at the given tabular location.
    pub async fn read_tabular(&self, location: &TabularLocation) -> Option<TextData> {
        self.handle.read_tabular(location).await
    }

    /// Apply a batch of tabular redactions to the document.
    pub async fn apply_tabular_redactions(
        &mut self,
        redactions: Redactions<TabularLocation, TabularRedaction>,
    ) -> Result<(), Error> {
        self.handle.apply_tabular_redactions(redactions).await
    }
}
