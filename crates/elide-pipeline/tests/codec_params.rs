//! The per-format codec knobs a request carries on [`CodecParams`]:
//! that a non-default one reaches the codec at all, and that it
//! survives the audit round-trip so anonymize decodes what analyze
//! decoded.

use bytes::Bytes;
use elide::PartId;
use elide::entity::LabelRef;
use elide::modality::tabular::Tabular;
use elide_pipeline::Engine;
use elide_pipeline::file::Document;
use elide_pipeline::provider::{CodecParams, Deployment, Provider, RequestContext};
use elide_schema::policy::{LabelScope, Policy};

/// A headerless CSV: every row is data, and the first row carries
/// an address like the others.
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
/// is observable: under the default the first row is the header, so
/// a cell below it is addressed by that header's text as its column
/// name. Told the file is headerless, every row is data and no cell
/// has a column name.
///
/// Both halves run the same bytes through the same policy, so the
/// knob is the only variable.
#[tokio::test]
async fn headerless_csv_names_no_columns() {
    let default = column_names_under(CodecParams::new()).await;
    let headerless = column_names_under(CodecParams::new().with_csv_has_headers(false)).await;

    assert!(
        default.iter().any(Option::is_some),
        "under the default the first row is a header naming the columns; got: {default:?}",
    );
    assert!(
        !headerless.is_empty() && headerless.iter().all(Option::is_none),
        "told the file is headerless, no cell has a column name; got: {headerless:?}",
    );
}

/// Analyze `HEADERLESS` under `codec`: the column name each detected
/// address is addressed by.
async fn column_names_under(codec: CodecParams) -> Vec<Option<String>> {
    let spec = RequestContext::new().with_codec(codec);
    let audit = engine()
        .analyze(csv(HEADERLESS), &[detect_email()], &spec)
        .await
        .expect("analyze succeeds")
        .audit;
    audit
        .report
        .part::<Tabular>(&PartId::leaf("contacts.csv"))
        .expect("the csv is a tabular document")
        .entities()
        .iter()
        .map(|entity| {
            entity
                .location()
                .column_name
                .as_ref()
                .map(ToString::to_string)
        })
        .collect()
}

/// The knobs ride the audit. Anonymize re-decodes from the audit
/// alone, so a param that did not survive serialization would let
/// the second decode disagree with the first, and the offsets
/// recorded against the first would land on different content.
#[test]
fn codec_params_survive_the_audit_round_trip() {
    let codec = CodecParams::new()
        .with_csv_has_headers(false)
        .with_csv_delimiter(b';');

    let json = serde_json::to_value(codec).expect("params serialize");
    let back: CodecParams = serde_json::from_value(json).expect("params deserialize");

    assert_eq!(back, codec, "every knob must survive the round trip");
}

/// An audit that names no codec params reads back as the defaults,
/// and those defaults are the codec's own behaviour: a header row for
/// CSV.
#[test]
fn omitted_params_default_to_the_codecs_own_behaviour() {
    let params = CodecParams::default();
    assert!(
        params.csv_has_headers,
        "a CSV's first row is its header unless a request says otherwise",
    );
    assert!(params.uses_default_formats());

    let back: CodecParams = serde_json::from_str("{}").expect("empty object deserializes");
    assert_eq!(back, params, "an omitted params object is the default one");
}

/// A format override takes the request off the shared registry.
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
}
