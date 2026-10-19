//! No credential reaches a debug rendering or a serialized
//! deployment by accident.
//!
//! A deployment is formatted whole in panics, `tracing` lines and
//! debug logs, so a derived `Debug` on a backend carrying an API
//! key puts that key in all of them. Each such backend has a
//! hand-written `Debug` instead; these assert it stays that way,
//! because adding a variant is exactly when the hand-written impl
//! gets forgotten.

/// A value no redaction could produce by coincidence.
const SECRET: &str = "sk-super-secret-value-9c3f";

/// Pioneer's API key never reaches a debug rendering.
#[cfg(feature = "pioneer")]
#[test]
fn the_pioneer_api_key_never_escapes() {
    use elide_deployment::component::NerBackend;

    let backend = NerBackend::Pioneer {
        api_key: SECRET.to_owned(),
        base_url: Some(format!("https://{SECRET}.example.com")),
        threshold: Some(0.7),
        zero_retention: true,
    };

    let rendered = format!("{backend:?}");
    assert!(
        !rendered.contains(SECRET),
        "the API key reached the debug output: {rendered}",
    );
    assert!(
        !rendered.contains("example.com"),
        "nor may the base URL, which can carry a credential of its own: {rendered}",
    );
    assert!(
        rendered.contains("zero_retention: true"),
        "a non-secret knob should still print, or the line is useless: {rendered}",
    );
}

/// An empty deployment serializes with no key-shaped field at all,
/// so a host that persists one cannot leak a credential it never
/// configured.
#[test]
fn an_unconfigured_deployment_serializes_no_credential_fields() {
    let json = serde_json::to_string(&elide_deployment::Deployment::default()).expect("serializes");

    for field in ["apiKey", "api_key", "secretAccessKey", "secret_access_key"] {
        assert!(
            !json.contains(field),
            "an empty deployment must not mention {field}: {json}",
        );
    }
}

/// Pioneer's key is skipped on serialize, and re-reading the
/// result **fails** rather than defaulting to an empty key.
///
/// The loudness is the point. `skip_serializing` without
/// `#[serde(default)]` makes a round-tripped config unloadable,
/// which is the intended trade: a secret is supplied, not
/// round-tripped. Adding `default` here would turn that loud
/// failure into an empty credential sent to the backend as a
/// credential — the silent-data-loss class this codebase refuses.
#[cfg(feature = "pioneer")]
#[test]
fn a_serialized_pioneer_config_cannot_be_reloaded_silently() {
    use elide_deployment::component::NerBackend;

    let backend = NerBackend::Pioneer {
        api_key: SECRET.to_owned(),
        base_url: None,
        threshold: None,
        zero_retention: false,
    };

    let json = serde_json::to_string(&backend).expect("serializes");
    assert!(
        !json.contains(SECRET),
        "the key must not be written: {json}"
    );

    let reloaded = serde_json::from_str::<NerBackend>(&json);
    let err = reloaded.expect_err("must not reload: the key was deliberately dropped");
    assert!(
        err.to_string().contains("api_key"),
        "and must say which field is missing: {err}",
    );
}

/// A parse error carries no source text, including when the
/// resolved secret itself breaks the TOML.
///
/// Interpolation runs before the parse, so the expanded document
/// holds the credential — and `toml`'s own `Display` quotes the
/// offending line to show where the fault is. That is right for a
/// config file in general and wrong here: a startup failure lands
/// in whatever collects logs.
///
/// A secret containing a quote is the sharper case, and the reason
/// this lives beside the other credential tests rather than with
/// the loaders: the quote terminates the TOML string early, so the
/// parse fails on a different construct than it would for a plain
/// value, reaching the error path by another route.
#[test]
fn a_parse_error_never_quotes_a_resolved_secret() {
    // Contains a quote, so expanding it breaks the string it sits in.
    let quoted = format!(r#"{SECRET}"and-more"#);

    let err = elide_deployment::Deployment::from_toml_resolved(
        "[[recognizers.llm]]\nname = \"x\"\nkind = \"open-ai\"\napi_key = \"${K}\"\nmodel = \"m\"\n",
        |_| Some(quoted.clone()),
    )
    .expect_err("the expanded document is no longer well-formed");

    assert_eq!(err.kind(), elide_deployment::ErrorKind::Malformed);
    assert!(
        !err.message().contains(SECRET),
        "the resolved secret reached the parse error: {err}",
    );
    assert!(
        err.message().contains("line 4"),
        "but the location must survive, or an operator cannot find the fault: {err}",
    );
}

/// A parse error withholds its detail when the document
/// interpolates, because even `toml`'s short message quotes the
/// offending *value*.
///
/// `invalid type: string "sk-…", expected f32` names the value, so
/// a secret interpolated into a field of the wrong type appeared
/// verbatim. A document that interpolates nothing keeps the full
/// message, since every byte the parser saw was already in the
/// operator's file.
///
/// Uses Pioneer's `threshold`, the one numeric backend field, so
/// the test needs that feature to have a type to mismatch.
#[cfg(feature = "pioneer")]
#[test]
fn a_type_error_withholds_an_interpolated_value() {
    // `threshold` wants a float; the resolved secret is a string.
    let interpolating = "[[recognizers.ner]]\nname = \"n\"\nkind = \"pioneer\"\n\
                         api_key = \"k\"\nthreshold = \"${K}\"\n";
    let err = elide_deployment::Deployment::from_toml_resolved(interpolating, |_| {
        Some(SECRET.to_owned())
    })
    .expect_err("a string is not an f32");

    assert!(
        !err.message().contains(SECRET),
        "the interpolated secret reached the error: {err}",
    );
    assert!(
        err.message().contains("line 1"),
        "the location must survive so an operator can find it: {err}",
    );

    // Nothing interpolated: the message is worth keeping, because
    // the value it quotes is one the operator wrote.
    let literal = "[[recognizers.ner]]\nname = \"n\"\nkind = \"pioneer\"\n\
                   api_key = \"k\"\nthreshold = \"not-a-float\"\n";
    let err = elide_deployment::Deployment::from_toml_resolved(literal, |_| None)
        .expect_err("still not an f32");

    assert!(
        err.message().contains("f32"),
        "a non-interpolating document keeps the detail: {err}",
    );
}
