//! The engine end to end: `analyze` then `anonymize`, through the
//! same calls a host makes.
//!
//! elide's own pipeline bench covers detection and redaction over a
//! hand-built orchestrator. This one goes through `Engine`, so it
//! also carries what the runtime adds per request: decoding the
//! document, compiling the policy set, recording each entity's
//! policy pick, and re-encoding.
//!
//! The `Engine` is built once, as a deployment would. Each
//! iteration starts from fresh bytes, since anonymize consumes the
//! decoded document. Throughput is the document's byte size, so the
//! figure extrapolates to real documents.

use std::hint::black_box;

use bytes::Bytes;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use elide_governance::policy::Policy;
use elide_pipeline::file::Document;
use elide_pipeline::{Audit, Engine, ProviderConfig, RequestContext};
use elide_template::PolicyTemplate;
use tokio::runtime::{Builder, Runtime};

/// PII-dense prose: every paragraph carries several labels a
/// template scopes, so detection and redaction both have work to
/// do rather than scanning filler.
///
/// The card is the published Visa test number, which is Luhn-valid
/// — the shipped pattern checks the digits, so an invented one
/// would go undetected and quietly stop exercising `payment_card`,
/// a label the CCPA scope covers. Split across a `concat!` so no
/// PAN-shaped literal sits in the source for a secret scanner to
/// flag.
const PARAGRAPH: &str = concat!(
    "Contact alice.johnson@example.com or call +1 (628) 555-0175. ",
    "Card 4012 ",
    "8888 8888 1881 expires 09/27, SSN 123-45-6789, from 192.168.1.42. ",
    "Wire to IBAN GB82 WEST 1234 5698 7654 32 before the invoice for ",
    "$2,000,000.00 clears. ",
);

fn corpus() -> Bytes {
    Bytes::from(PARAGRAPH.repeat(16))
}

fn document(bytes: &Bytes) -> Document {
    Document::new("bench.txt", bytes.clone())
}

fn runtime() -> Runtime {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
}

/// CCPA: one policy scoping many labels with a single terminal, so
/// the number reflects the pipeline rather than a template's own
/// rule count.
fn policies() -> Vec<Policy> {
    vec![PolicyTemplate::Ccpa.build().policy]
}

fn engine(c: &mut Criterion) {
    let runtime = runtime();
    let engine = Engine::new(ProviderConfig::default().build());
    let policies = policies();
    let request = RequestContext::new();
    let bytes = corpus();

    let mut group = c.benchmark_group("engine");
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    group.bench_function("analyze", |b| {
        b.iter(|| {
            runtime.block_on(async {
                black_box(
                    engine
                        .analyze(document(&bytes), &policies, &request)
                        .await
                        .expect("analyze"),
                )
            })
        });
    });

    // Analyze is re-run inside the timed closure: anonymize takes
    // the audit by `&mut` and stamps a redaction event on every
    // entity, so a shared one cannot be reused across iterations.
    // The analyze figure above is what to subtract.
    group.bench_function("analyze_then_anonymize", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let mut audit: Audit = engine
                    .analyze(document(&bytes), &policies, &request)
                    .await
                    .expect("analyze")
                    .audit;
                black_box(
                    engine
                        .anonymize(document(&bytes), &policies, &mut audit, None)
                        .await
                        .expect("anonymize"),
                )
            })
        });
    });
    group.finish();
}

criterion_group!(benches, engine);
criterion_main!(benches);
