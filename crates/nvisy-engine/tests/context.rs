//! Context storage CRUD and isolation tests.

mod fixtures;

use nvisy_ontology::context::Context;
use uuid::Uuid;

#[tokio::test]
async fn upload_download_roundtrip() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let ctx = Context::new("test-context", vec![]);
    let id = engine.registry().register_context(actor, ctx).await?.source().as_uuid();

    let downloaded = engine.registry().read_context(actor, id).await?.context().await?;
    assert_eq!(downloaded.name, "test-context");
    Ok(())
}

#[tokio::test]
async fn list_reflects_uploads() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let id = engine
        .registry()
        .register_context(actor, Context::new("ctx", vec![]))
        .await?
        .source()
        .as_uuid();

    let ids = engine.registry().list_contexts(actor).await?;
    assert!(ids.contains(&id));
    Ok(())
}

#[tokio::test]
async fn delete_removes_entry() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let id = engine
        .registry()
        .register_context(actor, Context::new("to-delete", vec![]))
        .await?
        .source()
        .as_uuid();
    engine.registry().unregister_context(actor, id).await?;

    let ids = engine.registry().list_contexts(actor).await?;
    assert!(!ids.contains(&id));
    Ok(())
}

#[tokio::test]
async fn delete_all_removes_everything() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    for i in 0..2 {
        engine
            .registry()
            .register_context(actor, Context::new(format!("ctx-{i}"), vec![]))
            .await?;
    }

    let removed = engine.registry().unregister_all_contexts(actor).await?;
    assert_eq!(removed, 2);
    assert!(engine.registry().list_contexts(actor).await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn download_nonexistent_returns_error() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let result = engine.registry().read_context(actor, Uuid::new_v4()).await;
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn actors_cannot_see_each_others_contexts() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor_a = fixtures::actor();
    let actor_b = fixtures::other_actor();

    let id = engine
        .registry()
        .register_context(actor_a, Context::new("private", vec![]))
        .await?
        .source()
        .as_uuid();

    assert!(engine.registry().list_contexts(actor_a).await?.contains(&id));
    assert!(!engine.registry().list_contexts(actor_b).await?.contains(&id));
    Ok(())
}

#[tokio::test]
async fn actors_cannot_download_each_others_contexts() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor_a = fixtures::actor();
    let actor_b = fixtures::other_actor();

    let id = engine
        .registry()
        .register_context(actor_a, Context::new("secret", vec![]))
        .await?
        .source()
        .as_uuid();

    let result = engine.registry().read_context(actor_b, id).await;
    assert!(result.is_err());
    Ok(())
}
