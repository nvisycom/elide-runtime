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
