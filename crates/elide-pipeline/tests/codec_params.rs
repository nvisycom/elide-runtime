//! The per-format codec knobs a request carries on [`CodecParams`]:
//! that a non-default one reaches the codec at all, and that it
//! survives the audit round-trip so anonymize decodes what analyze
//! decoded.

use bytes::Bytes;
use elide::entity::LabelRef;
use elide::redaction::HiddenContent;
use elide_pipeline::Engine;
use elide_pipeline::file::Document;
use elide_pipeline::provider::{
    CodecParams, Deployment, Disposition, HiddenContentPolicy, Provider, RequestContext,
};
use elide_schema::policy::{LabelScope, Policy};
use elide_schema::redaction::{ModalityRedactions, TabularRedaction, TextRedaction};

/// A headerless CSV: every row is data, and the first row carries
/// an address that must be reachable like any other.
const HEADERLESS: &[u8] = b"ada@example.com,1\ngrace@example.com,2\n";

/// An engine with no enrichers: these tests exercise the codec,
/// not detection backends.
fn engine() -> Engine {
    Engine::new(Provider::new(Deployment::default()))
}

/// A policy finding the fixture's addresses, so there is something
/// for a row-drop to act on.
fn detect_email() -> Policy {
    Policy {
        id: uuid::Uuid::now_v7(),
        name: "detect-email".into(),
        scopes: vec![LabelScope::new(
            "contact",
            vec![LabelRef::new("email_address")],
        )],
        ..Policy::default()
    }
}

/// The fixture as a named `.csv`, so the extension resolves to the
/// CSV codec.
fn csv(bytes: &'static [u8]) -> Document {
    Document::new("contacts.csv", Bytes::from_static(bytes))
}

/// `csv_has_headers: false` reaches the codec, and the difference
/// is observable: a header row is protected from `DropRow`, so
/// under the default the first record survives a drop-everything
/// policy. Told the file is headerless, the same row is ordinary
/// data and goes.
///
/// Both halves run the same bytes through the same policy, so the
/// knob is the only variable.
#[tokio::test]
async fn headerless_csv_lets_a_drop_reach_the_first_row() {
    let default_out = drop_rows_under(CodecParams::new()).await;
    let headerless_out = drop_rows_under(CodecParams::new().with_csv_has_headers(false)).await;

    assert!(
        default_out.contains("ada@example.com"),
        "under the default the first row is a protected header and must survive \
         the drop; got: {default_out:?}",
    );
    assert!(
        !headerless_out.contains("ada@example.com"),
        "told the file is headerless, the first row is droppable data and must \
         be dropped; got: {headerless_out:?}",
    );
}

/// Analyze then anonymize `HEADERLESS` under `codec`, with a policy
/// that drops every row carrying a detected address.
async fn drop_rows_under(codec: CodecParams) -> String {
    let engine = engine();
    let spec = RequestContext::new().with_codec(codec);
    let mut audit = engine
        .analyze(csv(HEADERLESS), &[detect_email()], &spec)
        .await
        .expect("analyze succeeds")
        .audit;

    let drop = Policy {
        id: uuid::Uuid::now_v7(),
        name: "drop-rows".into(),
        scopes: vec![LabelScope::new(
            "contact",
            vec![LabelRef::new("email_address")],
        )],
        fallback: Some(ModalityRedactions {
            tabular: Some(TabularRedaction::DropRow),
            ..Default::default()
        }),
        ..Policy::default()
    };

    let outcome = engine
        .anonymize(
            csv(HEADERLESS),
            std::slice::from_ref(&drop),
            &mut audit,
            None,
        )
        .await
        .expect("anonymize succeeds");
    String::from_utf8(outcome.bytes.to_vec()).expect("csv output is utf-8")
}

/// The knobs ride the audit. Anonymize re-decodes from the audit
/// alone, so a param that did not survive serialization would let
/// the second decode disagree with the first, and the offsets
/// recorded against the first would land on different content.
#[test]
fn codec_params_survive_the_audit_round_trip() {
    let codec = CodecParams::new()
        .with_csv_has_headers(false)
        .with_csv_delimiter(b';')
        .with_hidden_content(
            HiddenContentPolicy::default().with_collaboration(Disposition::Redact),
        );

    let json = serde_json::to_value(codec).expect("params serialize");
    let back: CodecParams = serde_json::from_value(json).expect("params deserialize");

    assert_eq!(back, codec, "every knob must survive the round trip");
}

/// An audit that names no codec params reads back as the defaults,
/// and those defaults are the codec's own behaviour: a header row for
/// CSV, and hidden content stripped.
#[test]
fn omitted_params_default_to_the_codecs_own_behaviour() {
    let params = CodecParams::default();
    assert!(
        params.csv_has_headers,
        "a CSV's first row is its header unless a request says otherwise",
    );
    assert_eq!(
        params.hidden_content.metadata,
        Disposition::Strip,
        "a request that expressed no opinion must not carry metadata through a redaction",
    );
    assert_eq!(
        params.hidden_content.collaboration,
        Disposition::Strip,
        "a request that expressed no opinion must not carry comments through a redaction",
    );
    assert!(params.uses_default_formats());

    let back: CodecParams = serde_json::from_str("{}").expect("empty object deserializes");
    assert_eq!(back, params, "an omitted params object is the default one");
}

/// A format override takes the request off the shared registry, and the
/// hidden-content policy does not: it goes to the decode itself, so asking
/// for it must not cost a registry rebuild.
#[test]
fn only_a_format_override_rebuilds_the_registry() {
    assert!(
        !CodecParams::new()
            .with_csv_has_headers(false)
            .uses_default_formats()
    );
    assert!(
        !CodecParams::new()
            .with_csv_delimiter(b'\t')
            .uses_default_formats()
    );
    assert!(
        CodecParams::new()
            .with_hidden_content(
                HiddenContentPolicy::default()
                    .with_metadata(Disposition::Redact)
                    .with_collaboration(Disposition::Redact),
            )
            .uses_default_formats()
    );
}

/// A page whose `<meta>` fields hold a name and an address, redacted under
/// `codec`: the items analyze reports stripped, and the page anonymize returns.
async fn redact_page_under(codec: CodecParams) -> (Vec<HiddenContent>, String) {
    const PAGE: &[u8] = b"<html><head>\
        <meta name=\"author\" content=\"Ada Lovelace\">\
        <meta name=\"contact\" content=\"ada@example.com\">\
        </head><body><p>Hello</p></body></html>";
    let page = || Document::new("page.html", Bytes::from_static(PAGE));
    let policy = Policy {
        fallback: Some(ModalityRedactions {
            text: Some(TextRedaction::Erase),
            ..Default::default()
        }),
        ..detect_email()
    };
    let policies = std::slice::from_ref(&policy);

    let engine = engine();
    let spec = RequestContext::new().with_codec(codec);
    let mut audit = engine
        .analyze(page(), policies, &spec)
        .await
        .expect("analyze succeeds")
        .audit;
    let stripped = audit.report.stripped().map(|item| item.content).collect();
    let outcome = engine
        .anonymize(page(), policies, &mut audit, None)
        .await
        .expect("anonymize succeeds");
    let html = String::from_utf8(outcome.bytes.to_vec()).expect("html output is utf-8");
    (stripped, html)
}

/// By default, metadata is stripped before detection sees it.
#[tokio::test]
async fn metadata_is_stripped_by_default() {
    let (stripped, html) = redact_page_under(CodecParams::new()).await;

    assert!(
        stripped.contains(&HiddenContent::Properties),
        "the strip is reported; got: {stripped:?}",
    );
    assert!(!html.contains("<meta"), "the fields are gone; got: {html}");
}

/// `Redact` reaches the codec on both decode paths, and anonymize decodes
/// under the policy the audit recorded: the fields survive to the output, and
/// detection has been through them. HTML carries metadata as text, so the
/// address is erased, and the author stays because this policy does not look
/// for names.
#[tokio::test]
async fn redacted_metadata_is_kept_and_goes_through_detection() {
    let redact = HiddenContentPolicy::default().with_metadata(Disposition::Redact);

    for codec in [
        CodecParams::new().with_hidden_content(redact),
        // A format override, so this one decodes through a rebuilt registry.
        CodecParams::new()
            .with_csv_has_headers(false)
            .with_hidden_content(redact),
    ] {
        let (stripped, html) = redact_page_under(codec).await;

        assert!(
            !stripped.contains(&HiddenContent::Properties),
            "the policy asked to redact, not strip; got: {stripped:?}",
        );
        assert!(html.contains("Ada Lovelace"), "kept; got: {html}");
        assert!(!html.contains("ada@example.com"), "detected; got: {html}");
    }
}

/// A pass that called a model records what it cost.
///
/// The whole point of wrapping each backend in `Metered` is that the
/// audit can answer "which model saw this data, and what did the
/// call cost". A mock backend still makes a call, so it still
/// records one — proving the sink is wired, not just present.
#[tokio::test]
async fn an_analyzed_pass_records_what_its_model_calls_cost() {
    use elide_pipeline::provider::NerBackend;

    let deployment = Deployment::builder()
        .ner("mock-ner", NerBackend::Mock)
        .finish()
        .build()
        .expect("one mock recognizer is a valid lineup");
    let engine = Engine::new(Provider::new(deployment));

    let audit = engine
        .analyze(csv(HEADERLESS), &[detect_email()], &RequestContext::new())
        .await
        .expect("analyze succeeds")
        .audit;

    assert!(
        !audit.usage.is_empty(),
        "a pass that ran a NER backend must record its call; got no entries",
    );
    // Named by the *configured* component name, not a fixed crate
    // string: an audit has to attribute a call to the component a
    // deployment wired, since two may share a backend kind.
    assert!(
        audit.usage.iter().all(|u| u.model.model == "mock-ner"),
        "every entry must name the configured component; got: {:?}",
        audit
            .usage
            .iter()
            .map(|u| &u.model.model)
            .collect::<Vec<_>>(),
    );
}
