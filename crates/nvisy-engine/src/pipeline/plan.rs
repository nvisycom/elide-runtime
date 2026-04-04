//! Graph compilation into an [`ExecutionPlan`].
//!
//! [`compile()`] bridges the user-facing [`Graph`] definition and the
//! runtime execution model. It validates the graph structure (via
//! [`Graph::validate`]) and checks for cycles (via petgraph
//! [`toposort`]), then walks the nodes to build a typed, phase-ordered
//! execution plan.
//!
//! All pipeline phases always run with default settings. User-provided
//! graph nodes customize *how* each phase runs, not *whether* it runs.
//! The plan is consumed by the [orchestrator] to drive per-document
//! execution.
//!
//! [`Graph::validate`]: nvisy_ontology::workflow::Graph::validate
//! [`toposort`]: petgraph::algo::toposort
//! [orchestrator]: super::orchestrator

use nvisy_core::{Error, Result};
use nvisy_ontology::workflow::{
    Detection, ExportFile, Extraction, Fusion, Graph, GraphNodeKind, ImportFile, Redaction,
    RetryPolicy, TimeoutPolicy, Validation,
};
use petgraph::algo::toposort;
use uuid::Uuid;

use crate::graph::GraphExt;

/// Import step — source node that creates envelopes.
#[derive(Debug, Clone)]
pub struct ImportStep {
    /// Import configuration (content IDs, decompression, decryption).
    pub config: ImportFile,
}

/// Export step — sink node that writes envelopes.
#[derive(Debug, Clone)]
pub struct ExportStep {
    /// Export configuration (content IDs, encryption, compression).
    pub config: ExportFile,
}

/// Per-phase retry and timeout policy overrides.
///
/// Carried on plan phases that involve external calls (extraction,
/// detection, redaction). Resolved from the user's graph node if
/// provided. Retry is applied to individual operation calls within
/// the phase; timeout wraps the entire phase.
#[derive(Debug, Clone, Default)]
pub struct PhasePolicy {
    /// Retry policy for individual operations within this phase.
    #[allow(dead_code)] // wired when operations gain internal retry
    pub retry: Option<RetryPolicy>,
    /// Timeout policy wrapping the entire phase.
    pub timeout: Option<TimeoutPolicy>,
}

/// Type-safe compiled execution plan.
///
/// All phases are present (non-optional) because they always run.
/// User-provided settings are merged in; defaults fill the gaps.
/// Redaction and post-redaction phases are skipped at runtime when
/// `dry_run` is true — the plan itself is always complete.
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    /// Content imports (phase 0).
    pub imports: Vec<ImportStep>,
    /// Context IDs to load into the cache (phase 0).
    pub context_ids: Vec<Uuid>,
    /// Extraction settings per modality (phase 1).
    pub extraction: Extraction,
    /// Retry/timeout for the extraction phase.
    pub extraction_policy: PhasePolicy,
    /// Detection settings — NER + Pattern (phase 2).
    pub detection: Detection,
    /// Retry/timeout for the detection phase.
    pub detection_policy: PhasePolicy,
    /// Fusion settings (phase 3).
    pub fusion: Fusion,
    /// Redaction settings (phase 4, skipped in dry-run).
    pub redaction: Redaction,
    /// Retry/timeout for the redaction phase.
    pub redaction_policy: PhasePolicy,
    /// Whether to generate context (phase 4).
    pub generate_context: bool,
    /// Validation settings (phase 5, skipped in dry-run).
    pub validation: Validation,
    /// Content exports (phase 6, skipped in dry-run).
    pub exports: Vec<ExportStep>,
    /// Context IDs to save (phase 6, skipped in dry-run).
    pub save_context_ids: Vec<Uuid>,
}

/// Compiles a [`Graph`] into an [`ExecutionPlan`].
///
/// Validates the graph structure and verifies it is a DAG (no cycles)
/// via petgraph topological sort, then walks nodes to build a typed
/// plan. Edges are consumed during validation and not preserved at
/// runtime.
pub(crate) fn compile(graph: &Graph) -> Result<ExecutionPlan> {
    graph.validate()?;

    // Verify the graph is acyclic via petgraph (belt-and-suspenders
    // with validate_dag in Graph::validate).
    let pg = graph.to_petgraph();
    toposort(&pg, None)
        .map_err(|_| Error::validation("graph contains a cycle", "compiler"))?;

    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut context_ids = Vec::new();
    let mut save_context_ids = Vec::new();
    let mut generate_context = false;
    let mut extraction = Extraction::default();
    let mut extraction_policy = PhasePolicy::default();
    let mut detection = Detection::default();
    let mut detection_policy = PhasePolicy::default();
    let mut fusion = Fusion::default();
    let mut redaction = Redaction::default();
    let mut redaction_policy = PhasePolicy::default();
    let mut validation = Validation::default();

    let mut has_extraction = false;
    let mut has_detection = false;
    let mut has_fusion = false;
    let mut has_redaction = false;
    let mut has_validation = false;

    for node in &graph.nodes {
        match &node.kind {
            GraphNodeKind::ImportFile(cfg) => {
                imports.push(ImportStep {
                    config: cfg.clone(),
                });
            }
            GraphNodeKind::ExportFile(cfg) => {
                exports.push(ExportStep {
                    config: cfg.clone(),
                });
            }
            GraphNodeKind::LoadContext(cfg) => {
                context_ids.extend_from_slice(&cfg.context_ids);
            }
            GraphNodeKind::SaveContext(cfg) => {
                save_context_ids.extend_from_slice(&cfg.context_ids);
            }
            GraphNodeKind::GenerateContext(_) => {
                generate_context = true;
            }
            GraphNodeKind::Extraction(cfg) => {
                if has_extraction {
                    return Err(Error::validation(
                        "graph may contain at most one Extraction node",
                        "compiler",
                    ));
                }
                extraction = cfg.clone();
                extraction_policy = PhasePolicy {
                    retry: node.retry.clone(),
                    timeout: node.timeout.clone(),
                };
                has_extraction = true;
            }
            GraphNodeKind::Detection(cfg) => {
                if has_detection {
                    return Err(Error::validation(
                        "graph may contain at most one Detection node",
                        "compiler",
                    ));
                }
                detection = cfg.clone();
                detection_policy = PhasePolicy {
                    retry: node.retry.clone(),
                    timeout: node.timeout.clone(),
                };
                has_detection = true;
            }
            GraphNodeKind::Fusion(cfg) => {
                if has_fusion {
                    return Err(Error::validation(
                        "graph may contain at most one Fusion node",
                        "compiler",
                    ));
                }
                fusion = cfg.clone();
                has_fusion = true;
            }
            GraphNodeKind::Redaction(cfg) => {
                if has_redaction {
                    return Err(Error::validation(
                        "graph may contain at most one Redaction node",
                        "compiler",
                    ));
                }
                redaction = cfg.clone();
                redaction_policy = PhasePolicy {
                    retry: node.retry.clone(),
                    timeout: node.timeout.clone(),
                };
                has_redaction = true;
            }
            GraphNodeKind::Validation(cfg) => {
                if has_validation {
                    return Err(Error::validation(
                        "graph may contain at most one Validation node",
                        "compiler",
                    ));
                }
                validation = cfg.clone();
                has_validation = true;
            }
            _ => {}
        }
    }

    context_ids.sort_unstable();
    context_ids.dedup();
    save_context_ids.sort_unstable();
    save_context_ids.dedup();

    Ok(ExecutionPlan {
        imports,
        context_ids,
        extraction,
        extraction_policy,
        detection,
        detection_policy,
        fusion,
        redaction,
        redaction_policy,
        generate_context,
        validation,
        exports,
        save_context_ids,
    })
}
