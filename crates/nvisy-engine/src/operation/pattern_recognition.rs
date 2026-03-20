//! Pattern-based entity recognition via regex, checksums, and dictionaries.

use futures::StreamExt;
use nvisy_codec::handler::{TextData, TextHandler};
use nvisy_codec::{Document, Span};
use nvisy_core::{Error, Result};
use nvisy_ontology::entity::{Entity, RecognitionMethod, TextLocation};

use crate::operation::envelope::DetectedEntities;
use crate::operation::{DocumentEnvelope, Operation, ParallelContext, SharedContext};

/// Pattern-based entity recognition using regex and dictionary matching.
pub struct PatternRecognition {
    shared: SharedContext,
}

impl PatternRecognition {
    async fn collect_spans(doc: &Document) -> Vec<Span<usize, TextData>> {
        match doc {
            Document::Text(h) => h.text_spans().await.collect().await,
            Document::Rich(h) => h.text_spans().await.collect().await,
            _ => Vec::new(),
        }
    }

    pub async fn connect(shared: SharedContext) -> Result<Self> {
        Ok(Self { shared })
    }

    pub(crate) async fn process(
        &self,
        mut envelope: DocumentEnvelope,
    ) -> Result<DocumentEnvelope, Error> {
        let spans = Self::collect_spans(&envelope.document).await;
        if spans.is_empty() {
            return Ok(envelope);
        }

        let entities = Self::scan(&spans);
        if !entities.is_empty() {
            envelope.apply(DetectedEntities(entities.into()));
        }
        Ok(envelope)
    }

    fn scan(spans: &[Span<usize, TextData>]) -> Vec<Entity> {
        let engine = nvisy_pattern::PatternEngine::instance();
        let scan_ctx = nvisy_pattern::ScanContext::default();
        let mut entities = Vec::new();

        for span in spans {
            let matches = engine.scan_text(span.data.as_str(), &scan_ctx);
            for m in matches {
                let entity = Entity::new(
                    m.category,
                    m.entity_kind,
                    &m.value,
                    RecognitionMethod::Regex,
                    m.confidence,
                )
                .with_location(
                    TextLocation {
                        start_offset: m.start,
                        end_offset: m.end,
                        element_id: Some(span.id.to_string()),
                        ..Default::default()
                    }
                    .into(),
                )
                .with_parent(&span.source);
                entities.push(entity);
            }
        }

        entities
    }
}

impl Operation for PatternRecognition {
    type Input = ParallelContext<Vec<Span<usize, TextData>>>;
    type Output = ParallelContext<DetectedEntities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input
            .parallel_map(|spans| async move { Ok(DetectedEntities(Self::scan(&spans).into())) })
            .await
    }
}
