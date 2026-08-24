//! Building a provider from a deployment's configuration, and
//! supplying a request's own key alongside it.
//!
//! The crate exists so the pipeline holds a running engine and
//! nothing about where its configuration came from, so what these
//! assert is that seam: a config round-trips as data and builds a
//! provider, while a key stays with the request that supplied it.

use elide_provider::{Component, KeyConfig, OcrBackend, OcrConfig, ProviderConfig, RequestContext};

const KEY: &[u8] = b"deployment-wide-key-32-bytes-ok!";

#[test]
fn an_empty_config_builds_a_provider_with_no_backends() {
    // The default is not an error: a deployment running only the
    // pattern recognizers elide ships configures nothing.
    let provider = ProviderConfig::default().build();
    assert!(
        provider.ner().recognizers.is_empty(),
        "no backends configured means none registered",
    );
    assert!(provider.ocr().enrichers.is_empty());
}

#[test]
fn configured_backends_reach_the_provider() {
    let config = ProviderConfig {
        ocr: OcrConfig {
            enrichers: vec![Component::<OcrBackend> {
                name: "acme-ocr".into(),
                description: None,
                backend: OcrBackend::Mock,
            }],
        },
        ..ProviderConfig::default()
    };

    let provider = config.build();
    let ocr = &provider.ocr().enrichers;
    assert_eq!(ocr.len(), 1, "the configured enricher is wired");
    assert_eq!(ocr[0].name.as_str(), "acme-ocr");
}

#[test]
fn a_config_round_trips_as_json() {
    // A host reads this from a file, or an encrypted row in its own
    // database; either way it is plain data.
    let config = ProviderConfig {
        ocr: OcrConfig {
            enrichers: vec![Component::<OcrBackend> {
                name: "acme-ocr".into(),
                description: Some("scanned intake forms".into()),
                backend: OcrBackend::Mock,
            }],
        },
        ..ProviderConfig::default()
    };

    let json = serde_json::to_string(&config).expect("config serializes");
    let back: ProviderConfig = serde_json::from_str(&json).expect("config deserializes");

    assert_eq!(back.ocr.enrichers.len(), 1);
    assert_eq!(back.ocr.enrichers[0].name.as_str(), "acme-ocr");
}

#[test]
fn a_provider_config_carries_no_key() {
    // The key belongs to the caller asking for redaction, not to
    // the deployment: one provider serves many callers, each with
    // its own. So a serialized provider config cannot leak one,
    // whatever the deployment does with it.
    let json = serde_json::to_string(&ProviderConfig::default()).expect("serializes");
    assert!(
        !json.contains("key"),
        "a provider config has no key field at all; got {json}",
    );
}

#[test]
fn a_request_carries_its_own_key() {
    let request = RequestContext::new().with_key(KeyConfig::Static { key: KEY.to_vec() });
    assert!(request.key.is_some());

    // And a request that needs none says so by supplying none.
    assert!(RequestContext::new().key.is_none());
}

#[test]
fn a_key_config_never_prints_its_material() {
    // `Debug` is what leaks a secret into a log or a trace, so it
    // prints the shape and nothing else.
    let config = KeyConfig::Static { key: KEY.to_vec() };
    let printed = format!("{config:?}");
    assert!(
        !printed.contains("key-32-bytes"),
        "no key material in Debug output; got {printed}",
    );
}
