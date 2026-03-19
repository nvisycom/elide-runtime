//! Entity recognition: NER, pattern matching, and manual detection.
//!
//! Combines all entity detection methods into operations that implement
//! [`NodeHandler`]. The NER agent, pattern engine, and manual annotation
//! converter are internal implementation details.

use futures::StreamExt;
use nvisy_codec::handler::{TextData, TextHandler, TxtSpan};
use nvisy_codec::{Document, Span};
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_http::HttpClient;
use nvisy_ontology::entity::{
    Annotation, AnnotationKind, Entities, Entity, EntityCategory, Location, RecognitionMethod,
    TextLocation,
};
use nvisy_rig::agent::{
    AgentConfig, AgentProvider, DetectionConfig, KnownNerEntity, NerAgent, NerContext,
};
use tokio::sync::Mutex;

use crate::graph::RetryPolicy;
use crate::operation::envelope::DetectedEntities;
use crate::operation::{DocumentEnvelope, NodeHandler, SequentialContext, SharedContext};
use crate::pipeline::RuntimeConfig;

const TARGET: &str = "nvisy_engine::op::recognition";

// ── Named Entity Recognition ──────────────────────────────────────

/// NER-based entity recognition. Wraps an [`NerAgent`] and carries
/// coreference state between spans via [`SequentialContext`].
pub struct EntityRecognition {
    agent: NerAgent,
    config: DetectionConfig,
    state: Mutex<Vec<KnownNerEntity>>,
    shared: SharedContext,
    retry: Option<RetryPolicy>,
}

impl EntityRecognition {
    fn build_agent(
        provider: &AgentProvider,
        config: AgentConfig,
        http_client: Option<HttpClient>,
    ) -> Result<NerAgent> {
        let agent = if let Some(client) = http_client {
            NerAgent::with_http_client(provider, config, client)
        } else {
            NerAgent::new(provider, config)
        }
        .map_err(|e| Error::validation(e.to_string(), "ner-agent"))?;
        Ok(agent)
    }

    async fn collect_spans(doc: &Document) -> Vec<Span<TxtSpan, String>> {
        let raw: Vec<Span<usize, TextData>> = match doc {
            Document::Text(h) => h.text_spans().await.collect().await,
            Document::Rich(h) => h.text_spans().await.collect().await,
            _ => return Vec::new(),
        };
        raw.into_iter()
            .map(|s| Span::new(TxtSpan(s.id), s.data.into_inner()).with_source(s.source))
            .collect()
    }

    /// Build from graph config and runtime dependencies.
    pub async fn connect(
        cfg: &crate::graph::NamedEntityRecognition,
        runtime: &RuntimeConfig,
        http_client: &HttpClient,
        shared: SharedContext,
        retry: Option<RetryPolicy>,
    ) -> Result<Self> {
        let llm = runtime.llm.as_ref();
        let provider = llm
            .and_then(|s| s.provider.clone())
            .ok_or_else(|| Error::new(ErrorKind::Validation, "NER requires an LLM provider"))?;
        let agent_config = llm.and_then(|s| s.policy.clone()).unwrap_or_default();

        let agent = Self::build_agent(&provider, agent_config, Some(http_client.clone()))?;
        let config = DetectionConfig {
            entity_kinds: cfg.entity_kinds.clone(),
            confidence_threshold: cfg.confidence_threshold.unwrap_or(0.5),
            system_prompt: None,
        };

        Ok(Self {
            agent,
            config,
            state: Mutex::new(Vec::new()),
            shared,
            retry,
        })
    }

    async fn detect(&self, spans: Vec<Span<TxtSpan, String>>) -> Result<DetectedEntities> {
        tracing::debug!(target: TARGET, span_count = spans.len(), "running NER");
        let mut entities = Vec::new();

        for span in &spans {
            let known = self.state.lock().await.clone();
            let ctx = NerContext::with_known(&span.data, known);

            let ner_entities = self
                .agent
                .detect(&ctx, &self.config)
                .await
                .map_err(|e| Error::runtime(e.to_string(), "ner-agent", e.is_retryable()))?;

            for ne in &ner_entities {
                let category: EntityCategory = match ne.category {
                    Some(c) => c,
                    None => continue,
                };
                let entity_kind = match ne.entity_type {
                    Some(ek) => ek,
                    None => continue,
                };
                let confidence = ne.confidence.unwrap_or(0.0);
                if confidence < self.config.confidence_threshold {
                    continue;
                }

                let mut entity = Entity::new(
                    category,
                    entity_kind,
                    &ne.value,
                    RecognitionMethod::Ner,
                    confidence,
                );
                let loc = if let Some(offsets) = ne.resolve_offsets(&ctx) {
                    TextLocation {
                        start_offset: offsets.start,
                        end_offset: offsets.end,
                        element_id: Some(span.id.0.to_string()),
                        ..Default::default()
                    }
                } else {
                    TextLocation {
                        element_id: Some(span.id.0.to_string()),
                        ..Default::default()
                    }
                };
                entity = entity.with_location(loc.into());
                entities.push(entity.with_parent(&span.source));
            }

            let mut state = self.state.lock().await;
            let mut merge_ctx = NerContext::with_known(&span.data, std::mem::take(&mut *state));
            merge_ctx.merge(ner_entities);
            *state = merge_ctx.known_entities;
        }

        Ok(DetectedEntities(entities.into()))
    }

    async fn reset(&self) {
        self.state.lock().await.clear();
    }
}

#[async_trait::async_trait]
impl NodeHandler for EntityRecognition {
    async fn handle(&self, mut envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        let spans = Self::collect_spans(&envelope.document).await;
        if !spans.is_empty() {
            let ner_ref = self;
            let retry = self.retry.as_ref();
            let do_ner = || {
                let spans = spans.clone();
                let shared = self.shared.clone();
                async move {
                    let input = SequentialContext::new(spans, shared);
                    ner_ref.detect(input.data).await
                }
            };
            let output = match retry {
                Some(policy) => policy.with_retry(do_ner).await?,
                None => do_ner().await?,
            };
            envelope.apply(output);
        }
        self.reset().await;
        Ok(envelope)
    }
}

// ── Pattern Recognition ───────────────────────────────────────────

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
}

#[async_trait::async_trait]
impl NodeHandler for PatternRecognition {
    async fn handle(&self, mut envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        let spans = Self::collect_spans(&envelope.document).await;
        if spans.is_empty() {
            return Ok(envelope);
        }

        let engine = nvisy_pattern::PatternEngine::instance();
        let scan_ctx = nvisy_pattern::ScanContext::default();
        let mut entities = Vec::new();

        for span in &spans {
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

        if !entities.is_empty() {
            envelope.apply(DetectedEntities(entities.into()));
        }
        Ok(envelope)
    }
}

/// Manual annotation: not a standalone NodeHandler, but a utility
/// for converting user-provided annotations into entities.
pub fn apply_manual_annotations(annotations: &[Annotation], entities: &mut Entities) {
    for ann in annotations {
        if ann.kind != AnnotationKind::Inclusion {
            continue;
        }
        let category = match ann.category {
            Some(c) => c,
            None => continue,
        };
        let entity_kind = match ann.entity_kind {
            Some(ek) => ek,
            None => continue,
        };
        let value = ann.value.clone().unwrap_or_default();
        let mut entity = Entity::new(category, entity_kind, value, RecognitionMethod::Manual, 1.0);
        entity.location = ann.location.clone();
        entities.push(entity);
    }
}

/// Check whether an entity falls within any exclusion annotation.
pub fn is_excluded(entity: &Entity, annotations: &[Annotation]) -> bool {
    for ann in annotations {
        if ann.kind != AnnotationKind::Exclusion {
            continue;
        }
        if let Some(ref excl_val) = ann.value
            && *excl_val == entity.value
        {
            return true;
        }
        if let (Some(Location::Text(entity_loc)), Some(Location::Text(excl_loc))) =
            (&entity.location, &ann.location)
            && entity_loc.overlaps(excl_loc)
        {
            return true;
        }
    }
    false
}
