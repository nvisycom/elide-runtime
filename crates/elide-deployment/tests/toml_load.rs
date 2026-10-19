//! Loading a deployment from TOML: that the file reaches the
//! config types intact, and that a missing variable fails loudly
//! rather than becoming an empty credential.
//!
//! Each document lives in `tests/testdata/`, so a fixture reads as
//! the file an operator would actually write.

use elide_deployment::component::{Availability, NerBackend};
use elide_deployment::{Deployment, ErrorKind};

/// One fixture from `tests/testdata/`.
macro_rules! testdata {
    ($name:literal) => {
        include_str!(concat!("testdata/", $name))
    };
}

/// The whole file reaches the config types: both recognizer
/// lineups, the enricher lineup, and the language candidates.
///
/// Asserted field by field rather than on a debug string, so a
/// field silently dropped by `flatten` is caught rather than
/// hidden in formatting.
#[test]
fn a_full_document_round_trips_into_the_config_types() {
    let deployment = Deployment::from_toml_resolved(testdata!("full.toml"), |key| {
        assert_eq!(key, "OPENAI_KEY", "only the one reference is looked up");
        Some("sk-test".to_owned())
    })
    .expect("loads");

    assert_eq!(deployment.recognizers().ner.len(), 1, "the NER lineup");
    assert_eq!(deployment.recognizers().llm.len(), 1, "the LLM lineup");
    assert_eq!(deployment.enrichers().ocr.len(), 1, "the OCR lineup");
    assert!(
        deployment.enrichers().stt.is_empty(),
        "no STT was configured"
    );

    let ner = &deployment.recognizers().ner[0];
    assert_eq!(ner.name, "medical-ner");
    assert_eq!(ner.tags, ["medical"]);
    assert_eq!(
        ner.backend,
        NerBackend::bento("https://ner.internal", "clinical"),
        "the flattened backend fields survive the nesting",
    );

    assert_eq!(
        deployment.enrichers().lingua.candidates,
        ["en", "es"],
        "language candidates reach the deployment, which is what lets a \
         deployment narrow detection at all",
    );
}

/// An unset variable fails the load. The alternative — expanding to
/// `""` — would send an empty credential to the backend and
/// surface as an authentication error far from its cause.
#[test]
fn an_unset_variable_fails_rather_than_becoming_empty() {
    let err = Deployment::from_toml_resolved(testdata!("full.toml"), |_| None)
        .expect_err("must not load");

    assert_eq!(err.kind(), ErrorKind::MissingVariable);
    assert!(
        err.message().contains("OPENAI_KEY"),
        "the message must name the variable so an operator can fix it: {err}",
    );
}

/// A document that configures nothing is a valid deployment: the
/// built-in pattern recognizers and language detection need no
/// configuration, so a host wanting only those writes an empty
/// file rather than being forced to name every empty lineup.
#[test]
fn an_empty_document_is_a_valid_deployment() {
    let deployment =
        Deployment::from_toml_resolved(testdata!("empty.toml"), |_| None).expect("loads");

    assert!(deployment.recognizers().ner.is_empty());
    assert!(deployment.recognizers().llm.is_empty());
    assert!(matches!(deployment.availability(), Availability::All));
    assert!(
        deployment.enrichers().lingua.is_unrestricted(),
        "unconfigured language detection stays unrestricted",
    );
}

/// A key the schema does not know is refused rather than ignored.
/// A typo'd `candidate` that silently did nothing would leave
/// detection unrestricted with no diagnostic.
#[test]
fn an_unknown_key_is_refused() {
    let err = Deployment::from_toml_resolved(testdata!("unknown_key.toml"), |_| None)
        .expect_err("must not load");

    assert_eq!(err.kind(), ErrorKind::Malformed);
    assert!(
        err.message().contains("candidate"),
        "the message must name the offending key: {err}",
    );
}

/// `$${VAR}` is the literal text, never a lookup. Without the
/// escape there is no way to write a value that contains `${`.
#[test]
fn a_doubled_dollar_escapes_the_interpolation() {
    let deployment =
        Deployment::from_toml_resolved(testdata!("escaped_interpolation.toml"), |key| {
            panic!("nothing should be looked up, got {key}")
        })
        .expect("loads");

    assert_eq!(
        deployment.recognizers().ner[0].backend,
        NerBackend::bento("${NOT_A_VAR}", "m"),
    );
}

/// Both rate limits reach the config, with `intervalSeconds`
/// defaulting so `requests` alone reads as per-minute.
#[test]
fn a_rate_limit_reaches_the_component() {
    let deployment =
        Deployment::from_toml_resolved(testdata!("rate_limited.toml"), |_| None).expect("loads");

    let limit = deployment.recognizers().ner[0]
        .rate_limit
        .expect("the component carries its limit");
    assert_eq!(limit.requests, 60);
    assert_eq!(limit.interval_seconds, 60, "defaults to per-minute");
    assert_eq!(limit.burst, Some(10));
    assert_eq!(limit.max_concurrent, Some(4));
}

/// A zero rate is refused at load. The decorators it configures
/// panic on zero, so accepting it would turn a typo in a config
/// file into a panic from inside a request.
#[test]
fn a_zero_rate_limit_is_refused_at_load() {
    let err = Deployment::from_toml_resolved(testdata!("zero_rate_limit.toml"), |_| None)
        .expect_err("must not load");

    assert_eq!(err.kind(), ErrorKind::Invalid);
    assert!(
        err.message().contains("never-runs") && err.message().contains("requests"),
        "the message must name the component and the field: {err}",
    );
}

/// Two components sharing a name is refused. A request selects by
/// name, so the duplicate is unreachable and every detection from
/// either would be attributed to the same name.
#[test]
fn a_duplicated_component_name_is_refused() {
    let err = Deployment::from_toml_resolved(testdata!("duplicate_names.toml"), |_| None)
        .expect_err("must not load");

    assert_eq!(err.kind(), ErrorKind::Invalid);
    assert!(
        err.message().contains("ambiguous"),
        "the message must name the collision: {err}",
    );
}

/// The hosted OCR backend names its cloud in the wire `kind`, so a
/// deployment file says whose infrastructure the document goes to.
/// `document_ai` alone would not.
#[cfg(feature = "google-document-ai")]
#[test]
fn the_cloud_ocr_backend_names_its_vendor() {
    use elide_deployment::component::OcrBackend;

    let deployment =
        Deployment::from_toml_resolved(testdata!("cloud_ocr.toml"), |_| None).expect("loads");

    assert_eq!(deployment.enrichers().ocr.len(), 1);
    assert!(
        matches!(
            deployment.enrichers().ocr[0].backend,
            OcrBackend::GoogleDocumentAi { .. }
        ),
        "kind = \"google_document_ai\" selects the Google backend",
    );
}

/// An unterminated `$${` is refused, and does not consume the next
/// real reference.
///
/// Regression: the escape branch searched for `}` across the whole
/// remaining document, so a `$${` with no closing brace of its own
/// ran to the *next* reference's `}` and emitted it literally. A
/// `base_url = "${REAL_URL}"` silently became the nine characters
/// `${REAL_URL}` — never looked up, no error — which is the
/// literal-credential version of the empty-credential failure this
/// expander exists to prevent.
#[test]
fn an_unterminated_escape_cannot_swallow_the_next_reference() {
    let mut looked_up = Vec::new();
    let result = Deployment::from_toml_resolved(testdata!("unterminated_escape.toml"), |key| {
        looked_up.push(key.to_owned());
        Some("https://resolved.example".to_owned())
    });

    let err = result.expect_err("an unterminated escape must be refused");
    assert_eq!(err.kind(), ErrorKind::Invalid);
    assert!(
        err.message().contains("unterminated"),
        "the message must say what is wrong: {err}",
    );
}

/// `validate` runs on the environment-backed loader too.
///
/// Regression: `from_toml` had its own body that parsed and
/// returned without validating, while `from_toml_resolved`
/// validated. A `requests = 0` therefore loaded through the path
/// real hosts use, and panicked later from inside a request —
/// `RateLimited::new` asserts the rate is non-zero. The two
/// loaders document identical error behaviour, so this pins it.
#[test]
fn the_environment_loader_validates_too() {
    let err = Deployment::from_toml(include_str!("testdata/zero_rate_limit.toml"))
        .expect_err("a zero rate must be refused on every path");

    assert_eq!(err.kind(), ErrorKind::Invalid);
    assert!(
        err.message().contains("never-runs"),
        "the message must name the component: {err}",
    );
}

/// Deserializing directly does not validate, and the error kinds
/// stay distinguishable.
///
/// `Deserialize` cannot report this crate's [`ErrorKind`], so
/// validating inside it would collapse `Invalid` into `Malformed`
/// and leave an operator unable to tell a syntax error from a rate
/// limit that can never admit a call. The loaders validate instead,
/// which is why a host that deserializes through serde itself must
/// call `validate`.
///
/// This pins both halves: the bypass exists, and it is the only
/// one.
#[test]
fn deserializing_directly_leaves_validation_to_the_caller() {
    let unvalidated: Deployment = toml::from_str(testdata!("zero_rate_limit.toml"))
        .expect("the document is well-formed, so parsing succeeds");

    let err = unvalidated
        .validate()
        .expect_err("but it describes a limiter that never admits a call");
    assert_eq!(
        err.kind(),
        ErrorKind::Invalid,
        "validating by hand keeps the semantic kind, unlike a parse error",
    );

    // The same document through a loader is refused with that kind
    // intact rather than wrapped as a syntax error.
    let loaded = Deployment::from_toml_resolved(testdata!("zero_rate_limit.toml"), |_| None)
        .expect_err("the loader validates");
    assert_eq!(loaded.kind(), ErrorKind::Invalid);
}

/// The file loader reads a deployment from disk and validates it,
/// exactly as the string loader does.
#[test]
fn the_file_loader_reads_and_validates() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testdata");

    let deployment = Deployment::from_toml_file(dir.join("empty.toml"))
        .expect("an empty document is a valid deployment");
    assert!(deployment.recognizers().ner.is_empty());

    let err = Deployment::from_toml_file(dir.join("zero_rate_limit.toml"))
        .expect_err("and the same validation applies");
    assert_eq!(err.kind(), ErrorKind::Invalid);
}

/// A path that cannot be read is reported as [`Unreadable`] and
/// names the path.
///
/// Naming it is the whole reason this constructor exists rather
/// than leaving the host to call `fs::read_to_string`: an operator
/// with several config files needs to know which one is missing.
///
/// [`Unreadable`]: ErrorKind::Unreadable
#[test]
fn an_unreadable_path_is_named_in_the_error() {
    let err = Deployment::from_toml_file("/nonexistent/elide/deployment.toml")
        .expect_err("the file does not exist");

    assert_eq!(
        err.kind(),
        ErrorKind::Unreadable,
        "a missing file is not a malformed document: nothing was parsed",
    );
    assert!(
        err.message().contains("/nonexistent/elide/deployment.toml"),
        "the message must name the path: {err}",
    );
}

/// A misspelled lineup key is refused, not ignored.
///
/// Regression: `Recognizers` and `Enrichers` lacked
/// `deny_unknown_fields`, so `[[enrichers.ocrr]]` parsed to an
/// empty deployment and reported success. A deployment believing
/// it had OCR configured would have had none, and every image
/// would come back with no text found — indistinguishable from an
/// image holding no text.
#[test]
fn a_misspelled_lineup_key_is_refused() {
    let err = Deployment::from_toml_resolved(testdata!("unknown_lineup.toml"), |_| None)
        .expect_err("must not load");

    assert_eq!(err.kind(), ErrorKind::Malformed);
    assert!(
        err.message().contains("ocrr") && err.message().contains("ocr"),
        "the message must name the typo and the valid keys: {err}",
    );
}

/// `rate_limit` is accepted beside `rateLimit`.
///
/// Regression: only the camelCase spelling parsed, so the
/// snake_case one — which matches every other key an operator
/// writes in this file — was an unknown field on a struct that
/// could not deny them, and the limit was silently dropped. A
/// deployment believing it was rate-limited was not.
#[test]
fn the_snake_case_rate_limit_key_is_accepted() {
    let deployment = Deployment::from_toml_resolved(testdata!("snake_rate_limit.toml"), |_| None)
        .expect("loads");

    let limit = deployment.recognizers().ner[0]
        .rate_limit
        .expect("the snake_case spelling must reach the component");
    assert_eq!(limit.requests, 30);
}

/// A parse error carries no source text.
///
/// Regression: `toml`'s own `Display` quotes the offending line,
/// and by the time parsing runs `${VAR}` has been expanded — so a
/// syntax error on the line holding a credential put that
/// credential in the error, and from there into whatever collects
/// startup logs.
#[test]
fn a_parse_error_does_not_quote_the_expanded_document() {
    const SECRET: &str = "sk-must-not-appear-in-the-error";

    let err = Deployment::from_toml_resolved(
        "[[recognizers.llm]]\nname = \"x\"\nkind = \"open-ai\"\napi_key = \"${K}\" oops\n",
        |_| Some(SECRET.to_owned()),
    )
    .expect_err("the document is malformed");

    assert_eq!(err.kind(), ErrorKind::Malformed);
    assert!(
        !err.message().contains(SECRET),
        "the expanded secret reached the error: {err}",
    );
    assert!(
        err.message().contains("line 4"),
        "but the location must survive, or an operator cannot find it: {err}",
    );
}

/// A malformed language tag is refused at load.
#[test]
fn a_malformed_language_tag_is_refused_at_load() {
    let err = Deployment::from_toml_resolved(
        "[enrichers.lingua]\ncandidates = [\"en\", \"not a language\"]\n",
        |_| None,
    )
    .expect_err("must not load");

    assert_eq!(err.kind(), ErrorKind::Invalid);
    assert!(
        err.message().contains("enrichers.lingua.candidates"),
        "the message must name the field: {err}",
    );
}

/// A misspelled *backend* key is refused, not ignored.
///
/// Regression: `Component` flattens its backend, and
/// `deny_unknown_fields` is rejected on the outer struct — the
/// flattened fields reach it first and look unknown. But the
/// attribute works on the enum itself, which is where a backend's
/// own keys are declared.
///
/// This one has privacy consequences: a deployment writing
/// `zero_retenton = true` believes it asked Pioneer not to retain
/// document text, and silently got the default.
#[cfg(feature = "pioneer")]
#[test]
fn a_misspelled_backend_key_is_refused() {
    let err = Deployment::from_toml_resolved(testdata!("backend_field_typo.toml"), |_| None)
        .expect_err("must not load");

    assert_eq!(err.kind(), ErrorKind::Malformed);
    assert!(
        err.message().contains("zero_retenton"),
        "the message must name the typo: {err}",
    );
    assert!(
        err.message().contains("zero_retention"),
        "and the key it was meant to be: {err}",
    );
}

/// A rate-limited STT enricher loads, keeping both the backend's
/// own fields and the component's.
///
/// `Component` consumes `name` and `rate_limit` before the
/// flattened `SttBackend` deserializes the rest, and the backend
/// enum now denies unknown fields — so this pins that the two
/// halves still agree on the enricher side. Every other rate-limit
/// fixture is a recognizer, and the only STT assertion elsewhere is
/// that the lineup is *empty*.
#[test]
fn a_rate_limited_stt_enricher_loads() {
    let deployment = Deployment::from_toml_resolved(testdata!("stt_rate_limited.toml"), |_| None)
        .expect("loads");

    let component = &deployment.enrichers().stt[0];
    assert_eq!(component.name, "audio");

    let limit = component
        .rate_limit
        .expect("the component carries its limit");
    assert_eq!(limit.requests, 30);
    assert_eq!(limit.max_concurrent, Some(2));
    assert_eq!(
        limit.interval_seconds, 60,
        "an omitted interval still reads as per-minute",
    );
}
