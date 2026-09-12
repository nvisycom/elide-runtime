//! Per-request policy compilation: turning a policy set into the
//! orchestrator that runs it.
//!
//! This is the cost the runtime adds over the toolkit. elide's own
//! benches time detection and redaction; none of that work starts
//! until the policies have been validated, flattened into a label
//! catalog, and compiled into rules — and that happens on *every*
//! request, since governance is re-derived rather than cached.
//!
//! The `Provider` is built once, as a deployment would. Only the
//! compile is timed, over the regulatory templates a real request
//! carries rather than a synthetic policy, so the numbers track
//! what the templates actually cost.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use elide_governance::policy::Policy;
use elide_governance::recognition::Recognition;
use elide_provider::{DocumentContext, Provider, ProviderConfig};
use elide_template::{
    GdprArticle9Treatment, GdprSensitiveScope, HipaaAccountNumbers, HipaaDeidMethod, PciDssPart,
    PolicyTemplate,
};
use uuid::Uuid;

/// The four shipped postures, as a deployment would submit them.
fn templates() -> Vec<(&'static str, Vec<Policy>)> {
    let hipaa = PolicyTemplate::HipaaDeidentification {
        method: HipaaDeidMethod::SafeHarbor,
        accounts: HipaaAccountNumbers::default(),
    };
    let gdpr = PolicyTemplate::GdprArticle9 {
        treatment: GdprArticle9Treatment::default(),
        scope: GdprSensitiveScope::default(),
    };
    let pci = PolicyTemplate::PciDss {
        part: PciDssPart::SavErase,
    };
    vec![
        ("hipaa_safe_harbor", vec![hipaa.build().policy]),
        ("gdpr_article9", vec![gdpr.build().policy]),
        ("pci_dss", vec![pci.build().policy]),
        ("ccpa", vec![PolicyTemplate::Ccpa.build().policy]),
    ]
}

/// Every template at once: the stacked posture a regulated
/// deployment actually runs, and the case where cross-policy
/// catalog merging does the most work.
fn stacked() -> Vec<Policy> {
    templates().into_iter().flat_map(|(_, p)| p).collect()
}

fn provider() -> Provider {
    ProviderConfig::default().build()
}

fn compile(c: &mut Criterion) {
    let provider = provider();
    let context = DocumentContext::default();
    // Templates introduce no vocabulary of their own: a regulatory
    // posture names shipped labels.
    let vocabulary: Vec<Recognition> = Vec::new();
    let correlation_id = Uuid::now_v7();

    let mut group = c.benchmark_group("compile");
    for (name, policies) in templates() {
        // Analyze compiles the catalog and the recognizer lineup.
        group.bench_with_input(BenchmarkId::new("analyze", name), &policies, |b, p| {
            b.iter(|| {
                black_box(
                    provider
                        .analyze_orchestrator(&context, &vocabulary, p, correlation_id)
                        .expect("analyze orchestrator"),
                )
            });
        });
        // Anonymize additionally compiles every rule and fallback
        // into operators, which is where the templates' size shows.
        group.bench_with_input(BenchmarkId::new("anonymize", name), &policies, |b, p| {
            b.iter(|| {
                black_box(
                    provider
                        .anonymize_orchestrator(&context, &vocabulary, p, None, correlation_id)
                        .expect("anonymize orchestrator"),
                )
            });
        });
    }

    // Both halves of the stacked set, so the combined posture is
    // measured the same way each template is: analyze merges four
    // catalogs, anonymize additionally compiles every rule.
    let all = stacked();
    group.bench_function("analyze/stacked", |b| {
        b.iter(|| {
            black_box(
                provider
                    .analyze_orchestrator(&context, &vocabulary, &all, correlation_id)
                    .expect("analyze orchestrator"),
            )
        });
    });
    group.bench_function("anonymize/stacked", |b| {
        b.iter(|| {
            black_box(
                provider
                    .anonymize_orchestrator(&context, &vocabulary, &all, None, correlation_id)
                    .expect("anonymize orchestrator"),
            )
        });
    });
    group.finish();
}

criterion_group!(benches, compile);
criterion_main!(benches);
