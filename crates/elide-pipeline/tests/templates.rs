//! End-to-end tests for the templates shipped in [`elide_template`].
//!
//! Each test submits a template through the full analyze →
//! anonymize pipeline against the plaintext sample and asserts
//! on the redacted output bytes. That's the strongest form of
//! coverage: proves the template's rule dispatch, the group
//! synthesis, and the operator wiring all agree end-to-end,
//! rather than any one layer in isolation.

use bytes::Bytes;
use elide_pipeline::file::Document;
use elide_pipeline::{Engine, KeyConfig, ProviderConfig, RequestContext};
use elide_template::{
    GdprArticle9Treatment, GdprSensitiveScope, HipaaAccountNumbers, HipaaDeidMethod, PciDssPart,
    PciPanRender, PolicyTemplate, Template,
};

const SAMPLE_TXT: &[u8] = include_bytes!("testdata/sample.txt");

fn engine() -> Engine {
    Engine::new(ProviderConfig::default().build())
}

/// The key a keyed-operator template redacts with. Supplied per
/// request, so the engine itself carries none.
fn key() -> KeyConfig {
    KeyConfig::Static {
        key: b"elide-template-test-key-32bytes!".to_vec(),
    }
}

fn raw_txt() -> Document {
    Document::new(Bytes::from_static(SAMPLE_TXT), "txt")
}

fn default_spec() -> RequestContext {
    RequestContext::new()
}

/// Run `template` through analyze + anonymize against the sample
/// and return the redacted body as a UTF-8 string.
async fn apply(engine: &Engine, template: Template, request: &RequestContext) -> String {
    let mut analyzed = engine
        .analyze(
            raw_txt(),
            std::slice::from_ref(&template.policy),
            &default_spec(),
        )
        .await
        .expect("analyze succeeds")
        .audit;
    let redacted = engine
        .anonymize(
            raw_txt(),
            std::slice::from_ref(&template.policy),
            &mut analyzed,
            request.key.as_ref(),
        )
        .await
        .expect("anonymize succeeds");
    String::from_utf8(redacted.bytes.to_vec()).expect("body is utf-8")
}

#[tokio::test]
async fn hipaa_safe_harbor_erases_contact_info_from_sample() {
    let template = PolicyTemplate::HipaaDeidentification {
        method: HipaaDeidMethod::SafeHarbor,
        accounts: HipaaAccountNumbers::Standard,
    }
    .build();
    let body = apply(&engine(), template, &RequestContext::new()).await;
    // The sample carries an email, a phone, and an SSN: every
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
async fn hipaa_limited_data_set_erases_contact_info_from_sample() {
    // The sample carries only labels that erase under both Safe
    // Harbor and LDS (email, phone, SSN, address, name). The
    // distinguishing survivor set for LDS (dates, ages, ZIP)
    // isn't in this sample: that split is proved by
    // `elide-template`'s unit tests. This test's job is to
    // confirm the LDS template wires end-to-end through the
    // engine and erases what §164.514(e)(2) tells it to.
    let template = PolicyTemplate::HipaaDeidentification {
        method: HipaaDeidMethod::LimitedDataSet,
        accounts: HipaaAccountNumbers::Standard,
    }
    .build();
    let body = apply(&engine(), template, &RequestContext::new()).await;
    assert!(
        !body.contains("jane.doe@example.com"),
        "email must be erased under HIPAA LDS; body was:\n{body}",
    );
    assert!(
        !body.contains("415-555-0142"),
        "phone must be erased under HIPAA LDS; body was:\n{body}",
    );
    assert!(
        !body.contains("123-45-6789"),
        "SSN must be erased under HIPAA LDS; body was:\n{body}",
    );
}

#[tokio::test]
async fn hipaa_expert_determination_pseudonymizes_contact_info_from_sample() {
    // Expert Determination's bulk terminal is Pseudonymize, so
    // the raw identifiers must be gone even though the surrogate
    // value is random. The unit tests prove the operator wiring;
    // this test confirms end-to-end that the scaffold analyzes
    // and anonymizes the sample.
    let template = PolicyTemplate::HipaaDeidentification {
        method: HipaaDeidMethod::ExpertDetermination,
        accounts: HipaaAccountNumbers::Standard,
    }
    .build();
    let body = apply(&engine(), template, &RequestContext::new()).await;
    assert!(
        !body.contains("jane.doe@example.com"),
        "email must not survive verbatim under HIPAA ED; body was:\n{body}",
    );
    assert!(
        !body.contains("415-555-0142"),
        "phone must not survive verbatim under HIPAA ED; body was:\n{body}",
    );
    assert!(
        !body.contains("123-45-6789"),
        "SSN must not survive verbatim under HIPAA ED; body was:\n{body}",
    );
}

#[tokio::test]
async fn gdpr_article_9_leaves_non_special_categories_alone() {
    // The sample contains contact info + an SSN but nothing from
    // the Article 9 special categories (no religion, ethnicity,
    // health data, biometric, sexual orientation, etc.). So the
    // GDPR template should redact nothing: the output equals
    // the input.
    let body = apply(
        &engine(),
        PolicyTemplate::GdprArticle9 {
            treatment: GdprArticle9Treatment::Erase,
            scope: GdprSensitiveScope::Article9,
        }
        .build(),
        &RequestContext::new(),
    )
    .await;
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
    // The sample has no `payment_card` entity: the PCI truncate
    // template targets exactly one label, so the sample should
    // round-trip unchanged.
    let body = apply(
        &engine(),
        PolicyTemplate::PciDss {
            part: PciDssPart::PanRender {
                render: PciPanRender::Truncate,
            },
        }
        .build(),
        &RequestContext::new(),
    )
    .await;
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
async fn pci_dss_sav_erase_leaves_non_sav_labels_alone() {
    // The sample has no `card_security_code` entity: the PCI SAV
    // template targets exactly one label, so the sample should
    // round-trip unchanged. (The distinguishing behavior: erasing
    // a real CVV: needs the richer sample fixture tracked in
    // #366.)
    let body = apply(
        &engine(),
        PolicyTemplate::PciDss {
            part: PciDssPart::SavErase,
        }
        .build(),
        &RequestContext::new(),
    )
    .await;
    assert!(
        body.contains("jane.doe@example.com"),
        "PCI SAV template doesn't cover email; must survive. Body:\n{body}",
    );
    assert!(
        body.contains("123-45-6789"),
        "PCI SAV template doesn't cover SSN; must survive. Body:\n{body}",
    );
}

#[tokio::test]
async fn pci_dss_pan_hmac_requires_key_provider() {
    // Without a key provider on the engine, compiling the HMAC
    // template's HmacHash operator fails at anonymize-time with
    // a Configuration error. Proves the template wires the right
    // capability requirement into place.
    let template = PolicyTemplate::PciDss {
        part: PciDssPart::PanRender {
            render: PciPanRender::HmacSha256,
        },
    }
    .build();
    let mut analyzed = engine()
        .analyze(
            raw_txt(),
            std::slice::from_ref(&template.policy),
            &default_spec(),
        )
        .await
        .expect("analyze succeeds without a key")
        .audit;
    let err = engine()
        .anonymize(
            raw_txt(),
            std::slice::from_ref(&template.policy),
            &mut analyzed,
            None,
        )
        .await
        .expect_err("anonymize must fail when the request supplies no key");
    assert!(
        err.to_string().contains("KeyProvider"),
        "expected error naming the missing KeyProvider; got: {err}",
    );
}

#[tokio::test]
async fn pci_dss_pan_hmac_runs_with_a_key_provider() {
    // Sample has no PAN, so the redacted body still matches the
    // input verbatim: the test is that anonymize succeeds when
    // the engine carries a KeyProvider, which is the only PCI-
    // template-specific setup the operator needs.
    let body = apply(
        &engine(),
        PolicyTemplate::PciDss {
            part: PciDssPart::PanRender {
                render: PciPanRender::HmacSha256,
            },
        }
        .build(),
        &RequestContext::new().with_key(key()),
    )
    .await;
    assert!(
        body.contains("jane.doe@example.com"),
        "sample carries no PAN; contact info must survive verbatim. Body:\n{body}",
    );
}

#[tokio::test]
async fn ccpa_erases_contact_info_and_identifiers_from_sample() {
    // CCPA §(A) identifiers include email, phone, and SSN -
    // every one shows up in the sample and should be redacted
    // under the shipped template. (Address is present too but
    // the spec runs NER off, so it isn't detected here.)
    let body = apply(
        &engine(),
        PolicyTemplate::Ccpa.build(),
        &RequestContext::new(),
    )
    .await;
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
