//! Building an engine from a deployment's configuration.
//!
//! The crate exists so `elide-pipeline` holds a running engine and
//! nothing about where its configuration came from, so what these
//! assert is the seam: a config round-trips as data, builds an
//! engine, and disagreements about key material fail at build
//! rather than at the first request that needs a key.

use elide::ErrorKind;
use elide_config::{EngineConfig, KeyConfig, Keyring};
use elide_pipeline::recognition::{OcrBackend, OcrConfig, OcrEnricherConfig};

const KEY: &[u8] = b"deployment-wide-key-32-bytes-ok!";

#[test]
fn an_empty_config_builds_an_engine_with_no_backends() {
    // The default is not an error: a deployment running only the
    // pattern recognizers elide ships configures nothing.
    let engine = EngineConfig::default()
        .build(&Keyring::new())
        .expect("an empty config builds");
    assert_eq!(
        engine.components().len(),
        0,
        "no backends configured means none registered",
    );
}

#[test]
fn configured_backends_reach_the_engine() {
    let config = EngineConfig {
        ocr: OcrConfig {
            enrichers: vec![OcrEnricherConfig {
                name: "acme-ocr".into(),
                description: None,
                backend: OcrBackend::Mock,
            }],
        },
        ..EngineConfig::default()
    };

    let engine = config.build(&Keyring::new()).expect("engine builds");
    let components = engine.components();
    assert_eq!(components.ocr.len(), 1, "the configured enricher is wired");
    assert_eq!(components.ocr[0].name.as_str(), "acme-ocr");
}

#[test]
fn a_config_round_trips_as_json() {
    // A host reads this from a file, or an encrypted row in its own
    // database; either way it is plain data.
    let config = EngineConfig {
        key: Some(KeyConfig::Static {
            secret: "redaction".into(),
        }),
        ocr: OcrConfig {
            enrichers: vec![OcrEnricherConfig {
                name: "acme-ocr".into(),
                description: Some("scanned intake forms".into()),
                backend: OcrBackend::Mock,
            }],
        },
        ..EngineConfig::default()
    };

    let json = serde_json::to_string(&config).expect("config serializes");
    let back: EngineConfig = serde_json::from_str(&json).expect("config deserializes");

    assert_eq!(
        back.key,
        Some(KeyConfig::Static {
            secret: "redaction".into()
        })
    );
    assert_eq!(back.ocr.enrichers.len(), 1);
    assert_eq!(back.ocr.enrichers[0].name.as_str(), "acme-ocr");
}

#[test]
fn key_material_is_not_a_config_field() {
    // The whole point of naming the provider rather than carrying
    // the key: a serialized config cannot leak one.
    let config = EngineConfig {
        key: Some(KeyConfig::Static {
            secret: "redaction".into(),
        }),
        ..EngineConfig::default()
    };
    let json = serde_json::to_string(&config).expect("config serializes");

    assert!(
        !json.contains("key-32-bytes"),
        "no key material can appear in a serialized config; got {json}",
    );
    assert!(
        json.contains("static"),
        "the config names the provider shape instead; got {json}",
    );
}

#[test]
fn a_named_secret_the_keyring_lacks_fails_at_build() {
    let config = EngineConfig {
        key: Some(KeyConfig::Static {
            secret: "redaction".into(),
        }),
        ..EngineConfig::default()
    };

    let Err(err) = config.build(&Keyring::new()) else {
        panic!("a key provider with no secret must not build silently");
    };
    assert_eq!(err.kind(), ErrorKind::Configuration, "{err}");
    assert!(
        err.to_string().contains("redaction"),
        "the error names the missing secret; got: {err}",
    );
}

#[test]
fn a_keyring_no_config_names_fails_at_build() {
    // The mirror case: supplying a key the engine would never use
    // means the deployment believes redaction is keyed when it is
    // not.
    let keyring = Keyring::new().with_secret("redaction", KEY);
    let Err(err) = EngineConfig::default().build(&keyring) else {
        panic!("an unused keyring must not build silently");
    };
    assert_eq!(err.kind(), ErrorKind::Configuration, "{err}");
    assert!(
        err.to_string().contains("names no key provider"),
        "the error says which half is missing; got: {err}",
    );
}

#[test]
fn a_caller_can_supply_its_own_key_provider() {
    // The escape hatch: a deployment whose keys are neither static
    // nor anything `KeyConfig` names implements `KeyProvider` and
    // hands the instance over.
    use std::sync::Arc;

    use elide::redaction::operators::{KeyProvider, StaticKey};

    let provider: Arc<dyn KeyProvider> = Arc::new(StaticKey::new(KEY.to_vec()));
    let engine = EngineConfig::default().build_with_key_provider(provider);
    assert_eq!(engine.components().len(), 0);
}
