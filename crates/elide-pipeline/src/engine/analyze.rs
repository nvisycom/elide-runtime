//! Detection: [`Engine::analyze`], its re-run counterpart
//! [`Engine::re_analyze`], and the driver both share.
//!
//! The two entry points differ only in what they seed enrichment
//! with — nothing on a first pass, a previous pass's artifacts on a
//! re-run — so they share one driver rather than two that can
//! drift.

use elide::{AnalyzedDocument, ArtifactSet, Directives, Error, ErrorKind, Result};
use elide_provider::{Analyzers, RequestContext};
use elide_schema::policy::Policy;

use super::Engine;
use super::report::has_documents;
use crate::audit::{Analyzed, Audit};
use crate::file::Document;

impl Engine {
    /// Analyze one document into an [`Audit`] and the enrichment
    /// beside it.
    ///
    /// Decodes `document`, drives [`Orchestrator::analyze`], and
    /// returns an [`Analyzed`]: the audit over the report it
    /// produced — every document *and* every part nested in one (DOCX
    /// embedded images, archive members, ...) — with the
    /// recognition context and what the pass cost, plus the
    /// [`ArtifactSet`] the pass extracted. The artifacts stay off
    /// the audit because they carry document content; see
    /// [`Analyzed`].
    ///
    /// `policies` contributes the label catalog that drives
    /// recognizer dispatch. Each carries its own [`LabelScope`]s,
    /// and a [`LabelInScope`] predicate can only name a group its
    /// own policy declared — enforced at request compile.
    ///
    /// The catalog is not persisted:
    /// [`anonymize`](Self::anonymize) re-derives it from the policy
    /// set it is handed. Pass the same set there, so rule bodies
    /// match against a catalog they helped define. See that method
    /// for how rules compose.
    ///
    /// # Overlap clustering
    ///
    /// elide clusters overlapping detections and picks one winner
    /// per cluster, stamping its attribution on every member. Two
    /// policies whose entities overlap can therefore each see the
    /// other's attribution on shared members. Per-policy cluster
    /// segmentation is a follow-up upstream.
    ///
    /// # Errors
    ///
    /// Returns [`Configuration`](ErrorKind::Configuration) for a
    /// policy that cannot compile, and
    /// [`MalformedInput`](ErrorKind::MalformedInput) for a document
    /// the codec cannot decode.
    ///
    /// [`LabelInScope`]: elide_schema::policy::Predicate::LabelInScope
    /// [`LabelScope`]: elide_schema::policy::LabelScope
    /// [`Orchestrator::analyze`]: elide::Orchestrator::analyze
    pub async fn analyze(
        &self,
        document: Document,
        policies: &[Policy],
        request: &RequestContext,
    ) -> Result<Analyzed> {
        // A first pass is a re-run seeded with nothing: every group
        // enriches from scratch. Sharing the driver keeps the two
        // entry points from drifting.
        self.drive(document, policies, request, &ArtifactSet::new())
            .await
    }

    /// Re-detect over `document`, seeding recognition with the
    /// enrichment from a previous pass so the OCR/transcript is
    /// reused instead of recomputed.
    ///
    /// The re-run counterpart to [`analyze`](Self::analyze), for
    /// detection separated in time from a first pass: after a
    /// review gap, re-run with an added recognizer or a narrowed
    /// policy set without paying for OCR/STT again — which also
    /// keeps entity offsets meaningful, since a second transcript
    /// need not agree with the first.
    ///
    /// `prior` must be the artifacts this same document produced.
    /// Seeding one document's enrichment into another's analysis
    /// yields entity offsets into content that is not there.
    ///
    /// # Errors
    ///
    /// As [`analyze`](Self::analyze).
    pub async fn re_analyze(
        &self,
        document: Document,
        policies: &[Policy],
        request: &RequestContext,
        prior: &ArtifactSet,
    ) -> Result<Analyzed> {
        self.drive(document, policies, request, prior).await
    }

    /// The shared driver behind [`analyze`](Self::analyze) and
    /// [`re_analyze`](Self::re_analyze): decode, detect seeded with
    /// `prior` (empty on a first pass), then record each entity's
    /// policy pick.
    async fn drive(
        &self,
        document: Document,
        policies: &[Policy],
        request: &RequestContext,
        prior: &ArtifactSet,
    ) -> Result<Analyzed> {
        let correlation_id = document.correlation_id;
        let extension = document.extension.clone();
        let mut handle = self.decode(document, request.codec).await?;
        let Analyzers {
            orchestrator,
            usage,
        } = self.provider().analyze_orchestrator(
            &request.context,
            &request.recognition,
            policies,
            correlation_id,
            &request.selection,
        )?;
        let AnalyzedDocument {
            mut report,
            artifacts,
        } = orchestrator
            .re_analyze(&mut handle, prior, &Directives::new())
            .await?;
        // Read after the analysis, not off the report: a model call
        // is recorded where it happens, by the `Metered` decorator
        // each configured backend is wrapped in, and the report
        // carries no usage of its own.
        let usage = usage.entries();

        // A document whose codec resolved to a modality with no
        // registered pipeline produces no part at all, and nothing
        // downstream can act on it.
        if !has_documents(&report) {
            return Err(Error::new(
                ErrorKind::CapabilityUnavailable,
                format!(
                    "codec resolved {extension:?} to a modality the orchestrator \
                     has no pipeline for"
                ),
            ));
        }

        // Record what each entity's policy pick would be, so the
        // returned audit answers "what happens to this, and why"
        // before a reviewer decides anything. Purely additive: it
        // appends Selection events and redacts nothing.
        //
        // A failure here does not fail the analyze. Every reason the
        // pick can fail — an unresolvable label, an operator whose
        // key has not arrived yet — is raised again by `anonymize`,
        // which compiles the same policies and does fail. The
        // keyless `HmacHash` case is the common one: the request
        // supplies its key at anonymize, so refusing to analyze
        // would deny the caller detections over a redaction they
        // have not asked for yet, and report the same fault twice.
        //
        // Scope-reference errors never reach here: `analyze_orchestrator`
        // rejects them above, before any of this runs.
        //
        // The observable signal is an audit carrying no `Selection`
        // events.
        let _: Result<()> = self.provider().record_picks(
            &request.context,
            &request.recognition,
            policies,
            correlation_id,
            &mut report,
        );

        Ok(Analyzed {
            audit: Audit {
                report,
                context: request.context.clone(),
                recognition: request.recognition.clone(),
                codec: request.codec,
                selection: request.selection.clone(),
                usage,
            },
            artifacts,
        })
    }
}
