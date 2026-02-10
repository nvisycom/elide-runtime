pub mod audit_store;
pub mod config;
pub mod engine_factory;
pub mod policy_store;
pub mod state;

use std::sync::Arc;

// Re-exports for convenience
pub use audit_store::AuditStore;
pub use config::ServerConfig;
pub use engine_factory::create_registry;
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
    registry: Arc<nvisy_core::registry::Registry>,
    run_manager: Arc<nvisy_engine::runs::RunManager>,
    policy_store: Arc<PolicyStore>,
    audit_store: Arc<AuditStore>
}
