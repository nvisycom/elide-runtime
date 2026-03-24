//! Engine construction and basic lifecycle tests.

mod fixtures;

use nvisy_engine::pipeline::{Engine, EngineSection, ResourceLimits, RuntimeConfig};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn temp_engine_points_to_temp_dir() {
    let (engine, dir) = fixtures::engine();
    assert_eq!(engine.data_dir(), dir.path());
}

#[test]
fn open_with_default_config() -> Result {
    let dir = tempfile::tempdir()?;
    let engine = Engine::open(dir.path(), RuntimeConfig::default())?;
    assert_eq!(engine.data_dir(), dir.path());
    Ok(())
}

#[test]
fn open_with_custom_limits() -> Result {
    let dir = tempfile::tempdir()?;
    let config = RuntimeConfig {
        engine: Some(EngineSection {
            limits: ResourceLimits {
                run_timeout_ms: Some(5000),
                channel_buffer: 64,
            },
            ..Default::default()
        }),
        ..Default::default()
    };
    let engine = Engine::open(dir.path(), config)?;
    assert_eq!(
        engine.config().engine.as_ref().unwrap().limits.channel_buffer,
        64
    );
    Ok(())
}
