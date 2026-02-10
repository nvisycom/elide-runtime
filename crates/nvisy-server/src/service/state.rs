//! Central application state shared across all HTTP handlers.

use std::sync::Arc;
use nvisy_engine::runs::RunManager;
use super::audit_store::AuditStore;
use super::policy_store::PolicyStore;

/// Shared application state passed to every Axum handler via [`axum::extract::State`].
///
/// Each field is wrapped in an [`Arc`] so cloning the state is cheap.
#[derive(Clone)]
pub struct AppState {
    /// Manages in-flight and completed pipeline runs.
    pub run_manager: Arc<RunManager>,
    /// In-memory store of policy definitions.
    pub policy_store: Arc<PolicyStore>,
    /// In-memory store of audit log entries.
    pub audit_store: Arc<AuditStore>,
}
