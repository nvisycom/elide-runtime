//! Text accessors for [`Document`].

use futures::StreamExt;
use nvisy_codec::handler::{Redactions, TextData, TextRedaction};
use nvisy_codec::{Located, Span};
use nvisy_core::Error;
use nvisy_ontology::entity::TextLocation;

use super::Document;

impl Document {
    /// Collect all text locations into a `Vec`.
    pub async fn collect_text_locations(&self) -> Vec<Located<TextLocation>> {
        self.handle.text_locations().collect().await
    }

    /// Read the text content at the given text location.
    pub async fn read_text(&self, location: &TextLocation) -> Option<TextData> {
        self.handle.read_text(location).await
    }

    /// Collect every text location together with its data, skipping
    /// locations the handler can't read. Used by detection ops that
    /// scan extracted text spans without caring about the underlying
    /// streaming machinery.
    pub async fn collect_text_spans(&self) -> Vec<Span<TextLocation, TextData>> {
        let locations = self.collect_text_locations().await;
        let mut spans = Vec::with_capacity(locations.len());
        for located in locations {
            if let Some(data) = self.read_text(&located.location).await {
                spans.push(Span::from_located(located, data));
            }
        }
        spans
    }

    /// Apply a batch of text redactions to the document.
    pub async fn apply_text_redactions(
        &mut self,
        redactions: Redactions<TextLocation, TextRedaction>,
    ) -> Result<(), Error> {
        self.handle.apply_text_redactions(redactions).await
    }
}
