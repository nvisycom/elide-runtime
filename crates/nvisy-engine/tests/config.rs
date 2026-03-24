//! Configuration parsing, validation, and example TOML tests.

mod fixtures;

use nvisy_engine::pipeline::{EngineSection, ResourceLimits, RuntimeConfig};
use validator::Validate;

#[test]
fn example_toml_parses() {
    let contents = include_str!("../../../Nvisy.example.toml");

    // Parse into a loose Value first to verify TOML syntax is valid,
    // then extract the engine section which is always feature-independent.
    // Subsystem provider sections (llm, stt, tts) use cfg-gated enum
    // variants that may not be available in the test binary.
    let table: toml::Table = toml::from_str(contents)
        .expect("Nvisy.example.toml should be valid TOML");

    assert!(table.contains_key("version"), "example should have version");
    assert!(table.contains_key("engine"), "example should have [engine]");
    assert!(table.contains_key("server"), "example should have [server]");

    // Verify the engine section parses into our struct.
    let engine_toml = toml::to_string(table.get("engine").unwrap()).unwrap();
    let engine: EngineSection = toml::from_str(&engine_toml)
        .expect("[engine] section should parse into EngineSection");
    engine.validate()
        .expect("[engine] section should pass validation");
}

#[test]
fn empty_toml_uses_defaults() {
    let config: RuntimeConfig = toml::from_str("").unwrap();
    assert!(config.engine.is_none());
    assert!(config.ocr.is_none());
    assert!(config.llm.is_none());
    assert!(config.stt.is_none());
    assert!(config.tts.is_none());
    assert!(config.validate().is_ok());
}

#[test]
fn validation_rejects_zero_channel_buffer() {
    let config = RuntimeConfig {
        engine: Some(EngineSection {
            limits: ResourceLimits {
                channel_buffer: 0,
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn validation_accepts_defaults() {
    let config = RuntimeConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn merge_overrides_present_sections() {
    let base = RuntimeConfig {
        engine: Some(EngineSection {
            limits: ResourceLimits {
                channel_buffer: 256,
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    };
    let overrides = RuntimeConfig {
        engine: Some(EngineSection {
            limits: ResourceLimits {
                channel_buffer: 64,
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    };

    let merged = base.merge(&overrides);
    assert_eq!(merged.engine.unwrap().limits.channel_buffer, 64);
}

#[test]
fn merge_falls_back_to_base() {
    let base = RuntimeConfig {
        engine: Some(EngineSection {
            limits: ResourceLimits {
                channel_buffer: 512,
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    };
    let overrides = RuntimeConfig::default();

    let merged = base.merge(&overrides);
    assert_eq!(merged.engine.unwrap().limits.channel_buffer, 512);
}
