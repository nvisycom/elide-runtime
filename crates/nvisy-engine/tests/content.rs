//! Content storage CRUD and isolation tests.

mod fixtures;

use uuid::Uuid;

#[tokio::test]
async fn upload_download_roundtrip() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let id = fixtures::upload_text(&engine, actor, "hello world").await;
    let content = engine
        .registry()
        .read_content(actor, id)
        .await?
        .content()
        .await?;
    assert_eq!(content.as_str().unwrap(), "hello world");
    Ok(())
}

#[tokio::test]
async fn list_reflects_uploads() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let id = fixtures::upload_text(&engine, actor, "test").await;
    let ids = engine.registry().list_content(actor).await?;
    assert!(ids.contains(&id));
    Ok(())
}

#[tokio::test]
async fn delete_removes_entry() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let id = fixtures::upload_text(&engine, actor, "to delete").await;
    engine.registry().unregister_content(actor, id).await?;

    let ids = engine.registry().list_content(actor).await?;
    assert!(!ids.contains(&id));
    Ok(())
}

#[tokio::test]
async fn delete_all_removes_everything() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    for i in 0..3 {
        fixtures::upload_text(&engine, actor, &format!("doc {i}")).await;
    }

    let removed = engine.registry().unregister_all_content(actor).await?;
    assert_eq!(removed, 3);
    assert!(engine.registry().list_content(actor).await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn download_nonexistent_returns_error() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let result = engine.registry().read_content(actor, Uuid::new_v4()).await;
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn delete_nonexistent_returns_error() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let result = engine
        .registry()
        .unregister_content(actor, Uuid::new_v4())
        .await;
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn actors_cannot_see_each_others_content() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor_a = fixtures::actor();
    let actor_b = fixtures::other_actor();

    let id = fixtures::upload_text(&engine, actor_a, "secret").await;

    assert!(engine.registry().list_content(actor_a).await?.contains(&id));
    assert!(!engine.registry().list_content(actor_b).await?.contains(&id));
    Ok(())
}

#[tokio::test]
async fn actors_cannot_download_each_others_content() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine();
    let actor_a = fixtures::actor();
    let actor_b = fixtures::other_actor();

    let id = fixtures::upload_text(&engine, actor_a, "private").await;
    let result = engine.registry().read_content(actor_b, id).await;
    assert!(result.is_err());
    Ok(())
}
