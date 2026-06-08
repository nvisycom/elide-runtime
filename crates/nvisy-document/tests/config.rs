//! Configuration parsing, validation, and example TOML tests.

use nvisy_document::pipeline::{EngineConfig, RuntimeConfig};
use validator::Validate;

#[test]
fn example_toml_parses() {
    let contents = include_str!("../../../Nvisy.example.toml");

    // Parse into a loose Value first to verify TOML syntax is valid,
    // then extract the engine section which is always feature-independent.
    // Subsystem provider sections (llm, stt) use cfg-gated enum
    // variants that may not be available in the test binary.
    let table: toml::Table =
        toml::from_str(contents).expect("Nvisy.example.toml should be valid TOML");

    assert!(table.contains_key("version"), "example should have version");
    assert!(table.contains_key("engine"), "example should have [engine]");
    assert!(table.contains_key("server"), "example should have [server]");

    // Verify the engine section parses into our struct.
    let engine_toml = toml::to_string(table.get("engine").unwrap()).unwrap();
    let engine: EngineConfig =
        toml::from_str(&engine_toml).expect("[engine] section should parse into EngineConfig");
    engine
        .validate()
        .expect("[engine] section should pass validation");
}

#[test]
fn empty_toml_uses_defaults() {
    let config: RuntimeConfig = toml::from_str("").unwrap();
    assert!(config.engine.is_none());
    assert!(config.extraction.is_none());
    assert!(config.detection.is_none());
    assert!(config.validate().is_ok());
}

#[test]
fn validation_accepts_defaults() {
    let config = RuntimeConfig::default();
    assert!(config.validate().is_ok());
}
