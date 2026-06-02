//! [`ValidationPhase`]: per-node check-pipeline driver.
//!
//! Builds the canonical [`CheckPipeline`] from
//! [`Validation`] (today: a single [`LeakCheck`] with the configured
//! [`Severity`]) and runs it against each modality node in the tree.
//! Aggregates findings; any [`Severity::Fail`] finding fails the
//! run with a validation error.
//!
//! [`CheckPipeline`]: crate::validation::CheckPipeline
//! [`LeakCheck`]: crate::validation::LeakCheck
//! [`Severity`]: crate::validation::Severity
//! [`Validation`]: crate::validation::Validation

use nvisy_core::{Error, Result};
use tracing::Instrument;

use crate::core::{DocumentTree, DocumentView, NodeMut, RunContext, SharedHandle};
use crate::pipeline::EngineInput;
use crate::validation::{
    CheckContext, CheckPipeline, Finding, FindingKind, LeakCheck, Severity, Validation,
};

const TARGET: &str = "nvisy_engine::validation";

/// Validation phase orchestrator.
pub struct ValidationPhase;

impl ValidationPhase {
    /// Build the phase. Stateless.
    pub fn new() -> Self {
        Self
    }

    /// Walk the tree and run the canonical check pipeline per node.
    /// Visits the root first, then iterates nested embedded documents.
    pub(crate) async fn apply(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "validation");
        let run_id = ctx.shared().run_id;
        let cfg = input.plan.validation.clone();
        // Snapshot the tree-owned handle so it doesn't conflict with
        // the per-node `&mut` borrows produced by `root_mut` /
        // `embeds_mut` further down.
        let handle = tree.handle.clone();
        async move {
            let mut findings = Vec::new();
            findings.extend(dispatch(tree.root_mut(), &handle, &cfg, run_id).await?);
            for node in tree.embeds_mut() {
                findings.extend(dispatch(node, &handle, &cfg, run_id).await?);
            }
            finalize(&findings)
        }
        .instrument(span)
        .await
    }
}

impl Default for ValidationPhase {
    fn default() -> Self {
        Self::new()
    }
}

async fn dispatch(
    node: NodeMut<'_>,
    handle: &SharedHandle,
    cfg: &Validation,
    run_id: uuid::Uuid,
) -> Result<Vec<Finding>> {
    Ok(match node {
        NodeMut::Text(doc) => {
            let view = DocumentView::new(doc, handle);
            let pipeline: CheckPipeline<_, _> =
                CheckPipeline::new().with_check(LeakCheck::new(cfg.leak_severity));
            let ctx = CheckContext::new(&view, handle).with_correlation_id(run_id);
            log_findings("text", pipeline.run(doc, &ctx).await)
        }
        NodeMut::Tabular(doc) => {
            let view = DocumentView::new(doc, handle);
            let pipeline: CheckPipeline<_, _> =
                CheckPipeline::new().with_check(LeakCheck::new(cfg.leak_severity));
            let ctx = CheckContext::new(&view, handle).with_correlation_id(run_id);
            log_findings("tabular", pipeline.run(doc, &ctx).await)
        }
        // Image and Audio have no canonical check today. The
        // pipeline is empty; nothing runs.
        NodeMut::Image(_) | NodeMut::Audio(_) => Vec::new(),
    })
}

/// Log per-node findings at the appropriate level and return them
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
