//! Engine construction and basic lifecycle tests.

mod fixtures;

use std::time::Duration;

use nvisy_engine::pipeline::{Engine, EngineConfig, ResourceLimits, RuntimeConfig};

#[tokio::test]
async fn temp_engine_points_to_temp_dir() {
    let (engine, dir) = fixtures::engine().await;
    assert_eq!(engine.data_dir(), dir.path());
}

#[tokio::test]
async fn open_with_default_config() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let engine = Engine::open(dir.path(), RuntimeConfig::default()).await?;
    assert_eq!(engine.data_dir(), dir.path());
    Ok(())
}

#[tokio::test]
async fn open_with_custom_limits() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let config = RuntimeConfig {
        engine: Some(EngineConfig {
            limits: ResourceLimits {
                run_timeout: Some(Duration::from_secs(5)),
            },
            ..Default::default()
        }),
        ..Default::default()
    };
    let engine = Engine::open(dir.path(), config).await?;
    assert_eq!(
        engine.config().engine.as_ref().unwrap().limits.run_timeout,
        Some(Duration::from_secs(5))
    );
    Ok(())
}
