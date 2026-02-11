//! Shared application services, configuration, and state.
//!
//! This module re-exports the primary service types and implements Axum's
//! [`FromRef`](axum::extract::FromRef) for each sub-state field so that
//! handlers can extract individual services directly.

pub mod audit_store;
pub mod config;
pub mod pipeline;
pub mod policy_store;
pub mod state;

use std::sync::Arc;

// Re-exports for convenience
pub use audit_store::AuditStore;
pub use config::ServerConfig;
pub use policy_store::PolicyStore;
pub use state::AppState;

macro_rules! impl_di {
    ($($f:ident: $t:ty),+) => {$(
        impl axum::extract::FromRef<AppState> for $t {
            fn from_ref(state: &AppState) -> Self {
                state.$f.clone()
            }
        }
    )+};
}

impl_di! {
    run_manager: Arc<nvisy_engine::runs::RunManager>,
    policy_store: Arc<PolicyStore>,
    audit_store: Arc<AuditStore>
}
