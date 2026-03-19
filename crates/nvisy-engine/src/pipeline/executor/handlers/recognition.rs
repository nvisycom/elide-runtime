//! Recognition handlers: NER and pattern matching.

use futures::StreamExt;
use nvisy_codec::handler::{TextData, TextHandler};
use nvisy_codec::{Document, Span};
use nvisy_core::{Error, ErrorKind};
use nvisy_http::HttpClient;

use super::super::handler::NodeHandler;
use super::retry;
use crate::graph;
use crate::operation::inference::{Ner, NerMethodParams};
use crate::operation::processing::{PatternDetectionParams, PatternMatch};
use crate::operation::{DocumentEnvelope, Operation, ParallelContext, SequentialContext, SharedContext};
use crate::pipeline::config::RuntimeConfig;
use crate::pipeline::policy::CompiledRetryPolicy;

pub(crate) struct NerHandler {
    ner: Ner,
    shared: SharedContext,
    retry: Option<CompiledRetryPolicy>,
}

impl NerHandler {
    pub async fn new(
        cfg: &graph::NamedEntityRecognition,
        config: &RuntimeConfig,
        http_client: &HttpClient,
        shared: SharedContext,
        retry: Option<CompiledRetryPolicy>,
    ) -> Result<Self, Error> {
        let llm_section = config.llm.as_ref();
        let provider = llm_section
            .and_then(|s| s.provider.clone())
            .ok_or_else(|| Error::new(ErrorKind::Validation, "ner requires an LLM provider in config"))?;
        let agent_config = llm_section
            .and_then(|s| s.policy.clone())
            .unwrap_or_default();

        let ner = Ner::connect(NerMethodParams {
            entity_kinds: cfg.entity_kinds.clone(),
            confidence_threshold: cfg.confidence_threshold.unwrap_or(0.5),
            provider: Some(provider),
            agent_config: Some(agent_config),
            http_client: Some(http_client.clone()),
        })
        .await?;

        Ok(Self { ner, shared, retry })
    }
}

#[async_trait::async_trait]
impl NodeHandler for NerHandler {
    async fn handle(&self, mut envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        let text_spans = collect_ner_spans(&envelope.document).await;
        if !text_spans.is_empty() {
            let ner_ref = &self.ner;
            let do_ner = || {
                let spans = text_spans.clone();
                let shared = self.shared.clone();
                async move {
                    let input = SequentialContext::new(spans, shared);
                    ner_ref.call(input).await
                }
            };
            let output = retry::call(self.retry.as_ref(), do_ner).await?;
            envelope.apply(output.into_inner());
        }
        self.ner.reset().await;
        Ok(envelope)
    }
}

pub(crate) struct PatternRecognitionHandler {
    pattern_match: PatternMatch,
    shared: SharedContext,
    retry: Option<CompiledRetryPolicy>,
}

impl PatternRecognitionHandler {
    pub async fn new(
        shared: SharedContext,
        retry: Option<CompiledRetryPolicy>,
    ) -> Result<Self, Error> {
        let pattern_match = PatternMatch::connect(PatternDetectionParams {
            confidence_threshold: 0.0,
            patterns: None,
        })
        .await?;

        Ok(Self { pattern_match, shared, retry })
    }
}

#[async_trait::async_trait]
impl NodeHandler for PatternRecognitionHandler {
    async fn handle(&self, mut envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        let text_spans = collect_text_spans(&envelope.document).await;
        if !text_spans.is_empty() {
            let pm_ref = &self.pattern_match;
            let do_pattern = || {
                let spans = text_spans.clone();
                let shared = self.shared.clone();
                async move {
                    let input = ParallelContext::new(spans, shared);
                    pm_ref.call(input).await
                }
            };
            let output = retry::call(self.retry.as_ref(), do_pattern).await?;
            envelope.apply(output.into_inner());
        }
        Ok(envelope)
    }
}

async fn collect_ner_spans(
    doc: &Document,
) -> Vec<Span<nvisy_codec::handler::TxtSpan, String>> {
    let raw: Vec<Span<usize, TextData>> = match doc {
        Document::Text(h) => h.text_spans().await.collect().await,
        Document::Rich(h) => h.text_spans().await.collect().await,
        _ => return Vec::new(),
    };
    raw.into_iter()
        .map(|s| {
            Span::new(nvisy_codec::handler::TxtSpan(s.id), s.data.into_inner())
                .with_source(s.source)
        })
        .collect()
}

async fn collect_text_spans(doc: &Document) -> Vec<Span<usize, TextData>> {
    match doc {
        Document::Text(h) => h.text_spans().await.collect().await,
        Document::Rich(h) => h.text_spans().await.collect().await,
        _ => Vec::new(),
    }
}
