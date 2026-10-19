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

/// AWS credentials never reach a debug rendering, including
/// through the enclosing backend's derived `Debug`.
#[cfg(feature = "aws-textract")]
#[test]
fn the_aws_credentials_never_escape() {
    use elide_deployment::component::{AwsCredentials, OcrBackend};

    let backend = OcrBackend::AwsTextract {
        region: Some("eu-west-1".to_owned()),
        credentials: Some(AwsCredentials {
            access_key_id: format!("AKIA{SECRET}"),
            secret_access_key: SECRET.to_owned(),
            session_token: Some(SECRET.to_owned()),
        }),
    };

    let rendered = format!("{backend:?}");
    assert!(
        !rendered.contains(SECRET),
        "a credential reached the debug output through the enclosing enum: {rendered}",
    );
    assert!(
        rendered.contains("eu-west-1"),
        "the region is not a secret and should still print: {rendered}",
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

/// The same for AWS credentials: skipped on the way out, and a
/// reload names the field rather than inventing an empty one.
#[cfg(feature = "aws-textract")]
#[test]
fn serialized_aws_credentials_cannot_be_reloaded_silently() {
    use elide_deployment::component::AwsCredentials;

    let creds = AwsCredentials {
        access_key_id: format!("AKIA{SECRET}"),
        secret_access_key: SECRET.to_owned(),
        session_token: None,
    };

    let json = serde_json::to_string(&creds).expect("serializes");
    assert!(!json.contains(SECRET), "no credential is written: {json}");

    let err = serde_json::from_str::<AwsCredentials>(&json).expect_err("must not reload silently");
    assert!(
        err.to_string().contains("access_key_id") || err.to_string().contains("secret_access_key"),
        "the error must name the missing credential: {err}",
    );
}
