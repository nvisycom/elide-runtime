//! The per-format codec knobs a request carries on [`CodecParams`]:
//! that a non-default one reaches the codec at all, and that it
//! survives the audit round-trip so anonymize decodes what analyze
//! decoded.

use bytes::Bytes;
use elide::entity::LabelRef;
use elide_governance::policy::{LabelScope, Policy};
use elide_governance::redaction::{ModalityRedactions, TabularRedaction};
use elide_pipeline::Engine;
use elide_pipeline::file::Document;
use elide_pipeline::provider::{CodecParams, ExifMetadata, ProviderConfig, RequestContext};

/// A headerless CSV: every row is data, and the first row carries
/// an address that must be reachable like any other.
const HEADERLESS: &[u8] = b"ada@example.com,1\ngrace@example.com,2\n";

/// An engine with no enrichers: these tests exercise the codec,
/// not detection backends.
fn engine() -> Engine {
    Engine::new(ProviderConfig::default().build())
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
        .with_exif_metadata(ExifMetadata::StripAll);

    let json = serde_json::to_value(codec).expect("params serialize");
    let back: CodecParams = serde_json::from_value(json).expect("params deserialize");

    assert_eq!(back, codec, "every knob must survive the round trip");
}

/// An audit that names no codec params reads back as the defaults,
/// and those defaults are the codec's own behaviour — a header row
/// for CSV and untouched EXIF — not the field types' own defaults.
#[test]
fn omitted_params_default_to_the_codecs_own_behaviour() {
    let params = CodecParams::default();
    assert!(
        params.csv_has_headers,
        "a CSV's first row is its header unless a request says otherwise",
    );
    assert_eq!(
        params.exif_metadata,
        ExifMetadata::Keep,
        "EXIF is kept unless a request asks for stripping",
    );
    assert!(params.is_default(), "the defaults must report as default");

    let back: CodecParams = serde_json::from_str("{}").expect("empty object deserializes");
    assert_eq!(back, params, "an omitted params object is the default one");
}

/// Setting any knob takes the request off the shared registry, so
/// the decode path knows to rebuild one.
#[test]
fn a_configured_param_is_not_default() {
    assert!(!CodecParams::new().with_csv_has_headers(false).is_default());
    assert!(!CodecParams::new().with_csv_delimiter(b'\t').is_default());
    assert!(
        !CodecParams::new()
            .with_exif_metadata(ExifMetadata::StripAll)
            .is_default()
    );
}
