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

use elide::Result;
use elide_deployment::Lingua;
use elide_deployment::component::{ComponentSelection, Resolver};
use elide_schema::recognition::Recognition;

use super::{Component, LlmBackend, NerBackend, OcrBackend, SttBackend};
#[cfg(feature = "google-document-ai")]
use crate::orchestrator::Clients;
use crate::orchestrator::ProviderInner;

/// What one request compiles its analyzers from.
///
/// Borrowed rather than owned: the configuration outlives the
/// request, so a `Compile` is made per request and discarded.
pub(crate) struct Compile<'a> {
    /// What the deployment wired: its recognizer and enricher
    /// lineups, and the clients built before any request arrived.
    provider: &'a ProviderInner,
    /// The vocabularies this request introduces.
    pub(crate) recognition: &'a [Recognition],
    /// Which components this caller may run, and which the request
    /// asked for. Folded in rather than passed beside: it is
    /// consulted at the same sites, over the same lineups, for the
    /// same request.
    resolver: Resolver<'a>,
}

impl<'a> Compile<'a> {
    /// The context for one request: what `provider` wired, the
    /// vocabularies the request introduces, and the components
    /// `selection` asks for under the provider's availability.
    pub(crate) fn new(
        provider: &'a ProviderInner,
        recognition: &'a [Recognition],
        selection: &'a ComponentSelection,
    ) -> Self {
        Self {
            provider,
            recognition,
            resolver: Resolver::new(&provider.availability, selection),
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
            .validate_keys(&self.provider.recognizers, &self.provider.enrichers)?;
        let ner: Vec<_> = self
            .resolver
            .resolve(&self.provider.recognizers.ner)
            .into_iter()
            .cloned()
            .collect();
        let llm: Vec<_> = self
            .resolver
            .resolve(&self.provider.recognizers.llm)
            .into_iter()
            .cloned()
            .collect();
        self.resolver
            .refuse_if_no_recognizer(&self.provider.recognizers, ner.len() + llm.len())?;
        Ok(Resolved {
            ner,
            llm,
            ocr: self
                .resolver
                .pick_one(&self.provider.enrichers.ocr, "OCR")?,
            stt: self
                .resolver
                .pick_one(&self.provider.enrichers.stt, "STT")?,
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
    /// The request this was resolved for: its vocabularies, and
    /// what the deployment wired.
    cx: &'a Compile<'a>,
}

impl<'a> Resolved<'a> {
    /// The vocabularies this request introduces.
    pub(crate) fn recognition(&self) -> &'a [Recognition] {
        self.cx.recognition
    }

    /// The clients built before this request arrived.
    #[cfg(feature = "google-document-ai")]
    pub(crate) fn clients(&self) -> &'a Clients {
        &self.cx.provider.clients
    }

    /// The language candidates the deployment narrowed detection to.
    ///
    /// Deployment-wide rather than resolved per request: there is
    /// one detector and it is always wired, so there is nothing to
    /// select between.
    pub(crate) fn lingua(&self) -> &'a Lingua {
        &self.cx.provider.enrichers.lingua
    }
}
