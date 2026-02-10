use std::sync::Arc;
use nvisy_engine::runs::RunManager;
use super::audit_store::AuditStore;
use super::policy_store::PolicyStore;
use nvisy_core::registry::Registry;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<Registry>,
    pub run_manager: Arc<RunManager>,
    pub policy_store: Arc<PolicyStore>,
    pub audit_store: Arc<AuditStore>,
}
