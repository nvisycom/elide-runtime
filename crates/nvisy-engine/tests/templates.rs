//! End-to-end tests for the templates shipped in [`nvisy_template`].
//!
//! Each test submits a template through the full analyze →
//! anonymize pipeline against the plaintext sample and asserts
//! on the redacted output bytes. That's the strongest form of
//! coverage: proves the template's rule dispatch, the group
//! synthesis, and the operator wiring all agree end-to-end,
//! rather than any one layer in isolation.

use std::sync::Arc;

use bytes::Bytes;
use nvisy_engine::{Engine, KeyProvider, StaticKey};
use nvisy_schema::file::Document;
use nvisy_schema::plan::{
    AnalyzerParams, EnricherParams, PatternRecognizerParams, ProviderSelection, RecognizerParams,
    ScopeParams,
};
use nvisy_template::Template;

const SAMPLE_TXT: &[u8] = include_bytes!("testdata/sample.txt");

fn engine() -> Engine {
    Engine::new()
}

fn engine_with_key() -> Engine {
    let provider: Arc<dyn KeyProvider> = Arc::new(StaticKey::new(*b"nvisy-template-test-key-32bytes"));
    Engine::new().with_key_provider(provider)
}

fn raw_txt() -> Document {
    Document::new(Bytes::from_static(SAMPLE_TXT), "txt")
}

fn default_spec() -> AnalyzerParams {
    AnalyzerParams {
        recognizers: RecognizerParams {
            pattern: Some(PatternRecognizerParams {
                builtins: true,
                context_enhanced: true,
                ..Default::default()
            }),
            ner: Some(ProviderSelection::All(false)),
            llm: Some(ProviderSelection::All(false)),
        },
        enrichers: EnricherParams::default(),
        deduplication: Default::default(),
        scope: ScopeParams::default(),
        annotations: Default::default(),
    }
}

/// Run `template` through analyze + anonymize against the sample
/// and return the redacted body as a UTF-8 string.
async fn apply(engine: &Engine, template: Template) -> String {
    let mut analyzed = engine
        .analyze(raw_txt(), &template.policies, &template.groups, &default_spec())
        .await
        .expect("analyze succeeds");
    let redacted = engine
        .anonymize(
            raw_txt(),
            &template.policies,
            &template.groups,
            &mut analyzed,
        )
        .await
        .expect("anonymize succeeds");
    String::from_utf8(redacted.bytes.to_vec()).expect("body is utf-8")
}

#[tokio::test]
async fn hipaa_safe_harbor_erases_contact_info_from_sample() {
    let body = apply(&engine(), nvisy_template::hipaa_safe_harbor()).await;
    // The sample carries an email, a phone, and an SSN — every
    // one falls in a HIPAA identifier category and should be
    // gone from the output. (Address is present too but the
    // spec runs NER off, so it isn't detected here.)
    assert!(
        !body.contains("jane.doe@example.com"),
        "email must be erased under HIPAA Safe Harbor; body was:\n{body}",
    );
    assert!(
        !body.contains("office@example.com"),
        "email must be erased under HIPAA Safe Harbor; body was:\n{body}",
    );
    assert!(
        !body.contains("415-555-0142"),
        "phone must be erased under HIPAA Safe Harbor; body was:\n{body}",
    );
    assert!(
        !body.contains("123-45-6789"),
        "SSN must be erased under HIPAA Safe Harbor; body was:\n{body}",
    );
}

#[tokio::test]
async fn gdpr_article_9_leaves_non_special_categories_alone() {
    // The sample contains contact info + an SSN but nothing from
    // the Article 9 special categories (no religion, ethnicity,
    // health data, biometric, sexual orientation, etc.). So the
    // GDPR template should redact nothing — the output equals
    // the input.
    let body = apply(&engine(), nvisy_template::gdpr_article_9()).await;
    assert!(
        body.contains("jane.doe@example.com"),
        "GDPR Article 9 doesn't cover email; must survive. Body:\n{body}",
    );
    assert!(
        body.contains("123-45-6789"),
        "GDPR Article 9 doesn't cover SSN; must survive. Body:\n{body}",
    );
}

#[tokio::test]
async fn pci_dss_pan_truncate_leaves_non_pan_labels_alone() {
    // The sample has no `payment_card` entity — the PCI truncate
    // template targets exactly one label, so the sample should
    // round-trip unchanged.
    let body = apply(&engine(), nvisy_template::pci_dss_pan_truncate()).await;
    assert!(
        body.contains("jane.doe@example.com"),
        "PCI PAN template doesn't cover email; must survive. Body:\n{body}",
    );
    assert!(
        body.contains("123-45-6789"),
        "PCI PAN template doesn't cover SSN; must survive. Body:\n{body}",
    );
}

#[tokio::test]
async fn pci_dss_pan_hmac_requires_key_provider() {
    // Without a key provider on the engine, compiling the HMAC
    // template's HmacHash operator fails at anonymize-time with
    // a Configuration error. Proves the template wires the right
    // capability requirement into place.
    let template = nvisy_template::pci_dss_pan_hmac();
    let mut analyzed = engine()
        .analyze(raw_txt(), &template.policies, &template.groups, &default_spec())
        .await
        .expect("analyze succeeds without a key provider");
    let err = engine()
        .anonymize(
            raw_txt(),
            &template.policies,
            &template.groups,
            &mut analyzed,
        )
        .await
        .expect_err("anonymize must fail without a key provider");
    assert!(
        err.to_string().contains("KeyProvider"),
        "expected error naming the missing KeyProvider; got: {err}",
    );
}

#[tokio::test]
async fn pci_dss_pan_hmac_runs_with_a_key_provider() {
    // Sample has no PAN, so the redacted body still matches the
    // input verbatim — the test is that anonymize succeeds when
    // the engine carries a KeyProvider, which is the only PCI-
    // template-specific setup the operator needs.
    let body = apply(&engine_with_key(), nvisy_template::pci_dss_pan_hmac()).await;
    assert!(
        body.contains("jane.doe@example.com"),
        "sample carries no PAN; contact info must survive verbatim. Body:\n{body}",
    );
}

#[tokio::test]
async fn ccpa_erases_contact_info_and_identifiers_from_sample() {
    // CCPA §(A) identifiers include email, phone, and SSN —
    // every one shows up in the sample and should be redacted
    // under the shipped template. (Address is present too but
    // the spec runs NER off, so it isn't detected here.)
    let body = apply(&engine(), nvisy_template::ccpa()).await;
    assert!(
        !body.contains("jane.doe@example.com"),
        "email is a CCPA §(A) identifier; must be erased. Body:\n{body}",
    );
    assert!(
        !body.contains("415-555-0142"),
        "phone is a CCPA §(A) identifier; must be erased. Body:\n{body}",
    );
    assert!(
        !body.contains("123-45-6789"),
        "SSN is a CCPA §(A) identifier; must be erased. Body:\n{body}",
    );
}

#[tokio::test]
async fn every_shipped_template_analyzes_and_anonymizes_the_sample() {
    // Smoke test: every template listed in `ALL` produces a
    // valid analyze/anonymize round-trip against the sample.
    // Catches wiring regressions (unknown group refs, unbuildable
    // operators) that unit tests inside nvisy-template can't
    // catch without an engine.
    let engine = engine_with_key();
    for (name, build) in nvisy_template::ALL {
        let template = build();
        let mut analyzed = engine
            .analyze(raw_txt(), &template.policies, &template.groups, &default_spec())
            .await
            .unwrap_or_else(|e| panic!("template `{name}` failed to analyze: {e}"));
        engine
            .anonymize(
                raw_txt(),
                &template.policies,
                &template.groups,
                &mut analyzed,
            )
            .await
            .unwrap_or_else(|e| panic!("template `{name}` failed to anonymize: {e}"));
    }
}
