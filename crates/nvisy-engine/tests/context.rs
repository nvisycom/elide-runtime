//! Context storage CRUD and isolation tests.

mod fixtures;

use nvisy_ontology::context::Context;
use uuid::Uuid;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn upload_download_roundtrip() -> Result {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let ctx = Context::new("test-context", vec![]);
    let id = engine.upload_context(actor, ctx).await?;

    let downloaded = engine.download_context(actor, id).await?;
    assert_eq!(downloaded.name, "test-context");
    Ok(())
}

#[tokio::test]
async fn list_reflects_uploads() -> Result {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let id = engine
        .upload_context(actor, Context::new("ctx", vec![]))
        .await?;

    let ids = engine.list_contexts(actor).await?;
    assert!(ids.contains(&id));
    Ok(())
}

#[tokio::test]
async fn delete_removes_entry() -> Result {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let id = engine
        .upload_context(actor, Context::new("to-delete", vec![]))
        .await?;
    engine.delete_context(actor, id).await?;

    let ids = engine.list_contexts(actor).await?;
    assert!(!ids.contains(&id));
    Ok(())
}

#[tokio::test]
async fn delete_all_removes_everything() -> Result {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    for i in 0..2 {
        engine
            .upload_context(actor, Context::new(format!("ctx-{i}"), vec![]))
            .await?;
    }

    let removed = engine.delete_all_contexts(actor).await?;
    assert_eq!(removed, 2);
    assert!(engine.list_contexts(actor).await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn download_nonexistent_returns_error() -> Result {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let result = engine.download_context(actor, Uuid::new_v4()).await;
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn actors_cannot_see_each_others_contexts() -> Result {
    let (engine, _dir) = fixtures::engine();
    let actor_a = fixtures::actor();
    let actor_b = fixtures::actor();

    let id = engine
        .upload_context(actor_a, Context::new("private", vec![]))
        .await?;

    assert!(engine.list_contexts(actor_a).await?.contains(&id));
    assert!(!engine.list_contexts(actor_b).await?.contains(&id));
    Ok(())
}

#[tokio::test]
async fn actors_cannot_download_each_others_contexts() -> Result {
    let (engine, _dir) = fixtures::engine();
    let actor_a = fixtures::actor();
    let actor_b = fixtures::actor();

    let id = engine
        .upload_context(actor_a, Context::new("secret", vec![]))
        .await?;

    let result = engine.download_context(actor_b, id).await;
    assert!(result.is_err());
    Ok(())
}
