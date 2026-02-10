pub mod audit;
pub mod graphs;
pub mod health;
pub mod policies;
pub mod redact;

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        health::health,
        health::ready,
        graphs::execute_graph,
        graphs::validate_graph,
        graphs::list_runs,
        graphs::get_run,
        graphs::cancel_run,
        redact::redact,
        policies::create_policy,
        policies::list_policies,
        policies::get_policy,
        policies::update_policy,
        policies::delete_policy,
        audit::list_audit,
        audit::get_audit_by_run,
    ),
    components(schemas(
        redact::RedactRequest,
        policies::CreatePolicyRequest,
        policies::UpdatePolicyRequest,
    ))
)]
pub struct ApiDoc;
