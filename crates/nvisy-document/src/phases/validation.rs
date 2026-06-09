//! [`ValidationPhase`]: per-document check-pipeline driver.
//!
//! Builds the canonical [`CheckPipeline`] from
//! [`Validation`] (today: a single [`LeakCheck`] with the configured
//! [`Severity`]) and runs it against each modality tree.
//! Aggregates findings; any [`Severity::Fail`] finding fails the
//! run with a validation error.
//!
//! [`CheckPipeline`]: nvisy_toolkit::validation::CheckPipeline
//! [`LeakCheck`]: nvisy_toolkit::validation::LeakCheck
//! [`Severity`]: nvisy_toolkit::validation::Severity
//! [`Validation`]: crate::pipeline::Validation

use nvisy_codec::core::IndexedHandle;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Audio, Image, Tabular, Text};
use nvisy_core::{Error, Result};
use nvisy_toolkit::validation::{
    CheckContext, CheckPipeline, Finding, FindingKind, LeakCheck, Severity,
};
use tracing::Instrument;

use crate::core::{DocumentTree, Plan, RunContext};
use crate::document::Document;
use crate::modality::DocumentModality;

const TARGET: &str = "nvisy_document::validation";

/// Validation phase orchestrator.
pub struct ValidationPhase;

impl ValidationPhase {
    /// Build the phase. Stateless.
    pub fn new() -> Self {
        Self
    }

    pub(crate) async fn apply_text(
        &self,
        ctx: &RunContext,
        plan: &Plan,
        tree: &mut DocumentTree<Text>,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "validation.text");
        let run_id = ctx.shared().run_id;
        let cfg = plan.validation.clone();
        async move {
            let redacted = stream_text(tree.handle.handler_mut()).await?;
            let entities = applied_entities(&tree.root);
            let pipeline: CheckPipeline<Text, DocumentTree<Text>> =
                CheckPipeline::new().with_check(LeakCheck::new(cfg.leak_severity));
            let mut check_ctx = CheckContext::new(&*tree).with_correlation_id(run_id);
            if let Some(ref text) = redacted {
                check_ctx = check_ctx.with_redacted_output(text);
            }
            let findings = log_findings("text", pipeline.run(&entities, &check_ctx).await);
            finalize(&findings)
        }
        .instrument(span)
        .await
    }

    pub(crate) async fn apply_tabular(
        &self,
        ctx: &RunContext,
        plan: &Plan,
        tree: &mut DocumentTree<Tabular>,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "validation.tabular");
        let run_id = ctx.shared().run_id;
        let cfg = plan.validation.clone();
        async move {
            let redacted = stream_tabular(tree.handle.handler_mut()).await?;
            let entities = applied_entities(&tree.root);
            let pipeline: CheckPipeline<Tabular, DocumentTree<Tabular>> =
                CheckPipeline::new().with_check(LeakCheck::new(cfg.leak_severity));
            let mut check_ctx = CheckContext::new(&*tree).with_correlation_id(run_id);
            if let Some(ref text) = redacted {
                check_ctx = check_ctx.with_redacted_output(text);
            }
            let findings = log_findings("tabular", pipeline.run(&entities, &check_ctx).await);
            finalize(&findings)
        }
        .instrument(span)
        .await
    }

    /// Image and audio have no canonical check today — succeed
    /// immediately so the pipeline orchestrator can call this
    /// uniformly per modality.
    pub(crate) async fn apply_image(
        &self,
        _ctx: &RunContext,
        _plan: &Plan,
        _tree: &mut DocumentTree<Image>,
    ) -> Result<()> {
        Ok(())
    }

    pub(crate) async fn apply_audio(
        &self,
        _ctx: &RunContext,
        _plan: &Plan,
        _tree: &mut DocumentTree<Audio>,
    ) -> Result<()> {
        Ok(())
    }
}

impl Default for ValidationPhase {
    fn default() -> Self {
        Self::new()
    }
}

/// Collect the entities whose redaction the audit reports as
/// `Applied` — the set the leak check must verify is no longer
/// present in the post-redaction output.
fn applied_entities<M: DocumentModality>(doc: &Document<M>) -> Vec<Entity<M>> {
    doc.audit
        .records
        .iter()
        .filter(|r| r.audit.as_ref().is_some_and(|e| e.execution.is_applied()))
        .map(|r| r.entity.clone())
        .collect()
}

/// Log per-tree findings at the appropriate level and return them
/// for run-level aggregation.
fn log_findings(modality: &'static str, findings: Vec<Finding>) -> Vec<Finding> {
    if findings.is_empty() {
        tracing::debug!(target: TARGET, modality, "validation passed");
        return findings;
    }
    let fail_count = findings
        .iter()
        .filter(|f| matches!(f.severity, Severity::Fail))
        .count();
    if fail_count > 0 {
        tracing::warn!(
            target: TARGET,
            modality,
            findings = findings.len(),
            failing = fail_count,
            "validation produced failing findings",
        );
    } else {
        tracing::warn!(
            target: TARGET,
            modality,
            findings = findings.len(),
            "validation produced warning findings",
        );
    }
    for finding in &findings {
        tracing::warn!(
            target: TARGET,
            severity = ?finding.severity,
            message = %finding.message,
            "validation finding",
        );
    }
    findings
}

/// Convert the collected findings into a phase result. Any
/// `Severity::Fail` finding fails the run with a validation error
/// listing all failing findings.
fn finalize(findings: &[Finding]) -> Result<()> {
    let failing: Vec<&Finding> = findings
        .iter()
        .filter(|f| matches!(f.severity, Severity::Fail))
        .collect();
    if failing.is_empty() {
        return Ok(());
    }
    let details = failing
        .iter()
        .map(|f| match &f.kind {
            FindingKind::Leak { entity_id, value } => format!("{value}({entity_id})"),
            FindingKind::Other => f.message.clone(),
            _ => f.message.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    Err(Error::validation(
        format!(
            "{} validation finding(s) failed the run: {}",
            failing.len(),
            details,
        ),
        "validation",
    ))
}

/// Drain the post-redaction text chunks into one concatenated string
/// for substring-based leak detection. Returns `None` when the handle
/// has no chunks at all.
async fn stream_text(handle: &mut dyn IndexedHandle<Text>) -> Result<Option<String>> {
    let mut buf = String::new();
    while let Some(chunk) = handle.next_chunk().await? {
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(chunk.data.as_str());
    }
    Ok((!buf.is_empty()).then_some(buf))
}

/// Drain post-redaction tabular cells, separating rows with newlines.
async fn stream_tabular(handle: &mut dyn IndexedHandle<Tabular>) -> Result<Option<String>> {
    let mut buf = String::new();
    let mut current_row: Option<u32> = None;
    while let Some(chunk) = handle.next_chunk().await? {
        let row = chunk.location.row_index;
        if let Some(prev) = current_row
            && prev != row
        {
            buf.push('\n');
        }
        if !buf.is_empty() && current_row == Some(row) {
            buf.push('\t');
        }
        buf.push_str(chunk.data.as_str());
        current_row = Some(row);
    }
    Ok((!buf.is_empty()).then_some(buf))
}
