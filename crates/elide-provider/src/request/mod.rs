//! [`Compile`]: everything one request needs to build its analyzers.
//!
//! The four modality compilers and every attach site below them used
//! to take the pieces individually — `ner`, `llm`, `ocr`, `stt`,
//! `recognition`, plus the availability and selection a resolver was
//! built from. Twelve parameters threaded through four parallel
//! paths, and each new per-request concern added a thirteenth.
//!
//! They are one value because they share one lifetime: a request
//! decides all of them at once, every compiler reads them together,
//! and none changes while an analyzer is being built.

use std::sync::Arc;

use elide::Result;
use elide::backend::metrics::{Usage, UsageCollector, UsageSink};
use elide_schema::recognition::Recognition;

use super::{Component, Enrichers, LlmBackend, NerBackend, OcrBackend, Recognizers, SttBackend};
use crate::component::{Availability, ComponentSelection, Resolver};

/// What one request compiles its analyzers from.
///
/// Borrowed rather than owned: the configuration outlives the
/// request, so a `Compile` is made per request and discarded.
pub(crate) struct Compile<'a> {
    /// The recognizer lineups a deployment configured.
    pub(crate) recognizers: &'a Recognizers,
    /// The enricher lineups a deployment configured.
    pub(crate) enrichers: &'a Enrichers,
    /// The vocabularies this request introduces.
    pub(crate) recognition: &'a [Recognition],
    /// Which components this caller may run, and which the request
    /// asked for. Folded in rather than passed beside: it is
    /// consulted at the same sites, over the same lineups, for the
    /// same request.
    resolver: Resolver<'a>,
    /// Where each backend call records what it cost.
    ///
    /// Owned, not borrowed, and cloned into every [`Metered`]
    /// wrapper: the collector shares its entries behind an `Arc`, so
    /// each clone writes to the same run's tally.
    ///
    /// [`Metered`]: elide::backend::metrics::Metered
    collector: UsageCollector,
}

impl<'a> Compile<'a> {
    /// The context for one request.
    pub(crate) fn new(
        recognizers: &'a Recognizers,
        enrichers: &'a Enrichers,
        recognition: &'a [Recognition],
        availability: &'a Availability,
        selection: &'a ComponentSelection,
    ) -> Self {
        Self {
            recognizers,
            enrichers,
            recognition,
            resolver: Resolver::new(availability, selection),
            collector: UsageCollector::new(),
        }
    }

    /// A sink for one backend, to hand [`Metered::new`].
    ///
    /// A fresh handle each call, all writing to this request's one
    /// tally — so a document analyzed by several backends reports
    /// every call, and two concurrent requests never mix.
    ///
    /// [`Metered::new`]: elide::backend::metrics::Metered::new
    pub(crate) fn sink(&self) -> Arc<dyn UsageSink> {
        Arc::new(self.collector.clone())
    }

    /// A handle on this request's tally, readable after the
    /// analysis has run.
    ///
    /// Owned rather than borrowed: the caller reads the tally once
    /// the orchestrator has finished, by which point the `Compile`
    /// it was built from is gone.
    pub(crate) fn usage_handle(&self) -> UsageHandle {
        UsageHandle {
            collector: self.collector.clone(),
        }
    }

    /// Resolve the lineups once, into the view the four modality
    /// compilers read.
    ///
    /// Once rather than per modality: every analyzer draws from the
    /// same lineups, so resolving inside each would repeat the work
    /// and let them disagree.
    ///
    /// # Errors
    ///
    /// Refuses a selection naming a component this caller may not
    /// use, one matching nothing registered, one that narrowed every
    /// recognizer away, or an enricher lineup that stayed ambiguous.
    pub(crate) fn resolve(&'a self) -> Result<Resolved<'a>> {
        // Keys are checked against every lineup before any is
        // filtered: a key naming a NER component is legitimate even
        // though the LLM lineup has never heard of it.
        self.resolver
            .validate_keys(self.recognizers, self.enrichers)?;
        let ner: Vec<_> = self
            .resolver
            .resolve(&self.recognizers.ner)
            .into_iter()
            .cloned()
            .collect();
        let llm: Vec<_> = self
            .resolver
            .resolve(&self.recognizers.llm)
            .into_iter()
            .cloned()
            .collect();
        self.resolver
            .refuse_if_no_recognizer(self.recognizers, ner.len() + llm.len())?;
        Ok(Resolved {
            ner,
            llm,
            ocr: self.resolver.pick_one(&self.enrichers.ocr, "OCR")?,
            stt: self.resolver.pick_one(&self.enrichers.stt, "STT")?,
            cx: self,
        })
    }
}

/// The lineups one request runs, resolved, with the context they
/// were resolved from.
///
/// What the four modality compilers read: each takes this one value
/// rather than the pieces, so adding a per-request concern is a
/// field here instead of a parameter on every signature.
pub(crate) struct Resolved<'a> {
    /// The NER recognizers that will run.
    pub(crate) ner: Vec<Component<NerBackend>>,
    /// The LLM recognizers that will run.
    pub(crate) llm: Vec<Component<LlmBackend>>,
    /// The single OCR enricher, if one survived.
    pub(crate) ocr: Option<&'a Component<OcrBackend>>,
    /// The single STT enricher, if one survived.
    pub(crate) stt: Option<&'a Component<SttBackend>>,
    /// The request this was resolved for: its vocabularies, and the
    /// sink each backend records into.
    cx: &'a Compile<'a>,
}

impl<'a> Resolved<'a> {
    /// The vocabularies this request introduces.
    pub(crate) fn recognition(&self) -> &'a [Recognition] {
        self.cx.recognition
    }

    /// A usage sink for one backend.
    pub(crate) fn sink(&self) -> Arc<dyn UsageSink> {
        self.cx.sink()
    }
}

/// A request's usage tally.
///
/// Handed back beside the analyzers so a caller can read what the
/// model calls cost once they have run, without holding the
/// per-request context they were compiled from.
#[derive(Debug, Clone)]
pub struct UsageHandle {
    /// Shares its entries with every sink wired into a backend.
    collector: UsageCollector,
}

impl UsageHandle {
    /// Every call recorded so far, in the order the calls completed.
    ///
    /// One entry per backend call, not per component: a component
    /// called twice for one document reports twice, and a failed
    /// call reports the wall-clock time it still cost.
    #[must_use]
    pub fn entries(&self) -> Vec<Usage> {
        self.collector.entries()
    }

    /// Whether any model call was recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.collector.entries().is_empty()
    }
}
