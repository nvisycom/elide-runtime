//! Content storage CRUD and isolation tests.

mod fixtures;

use uuid::Uuid;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn upload_download_roundtrip() -> Result {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let id = fixtures::upload_text(&engine, actor, "hello world").await;
    let content = engine.download_content(actor, id).await?;
    assert_eq!(content.as_str().unwrap(), "hello world");
    Ok(())
}

#[tokio::test]
async fn list_reflects_uploads() -> Result {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let id = fixtures::upload_text(&engine, actor, "test").await;
    let ids = engine.list_content(actor).await?;
    assert!(ids.contains(&id));
    Ok(())
}

#[tokio::test]
async fn delete_removes_entry() -> Result {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let id = fixtures::upload_text(&engine, actor, "to delete").await;
    engine.delete_content(actor, id).await?;

    let ids = engine.list_content(actor).await?;
    assert!(!ids.contains(&id));
    Ok(())
}

#[tokio::test]
async fn delete_all_removes_everything() -> Result {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    for i in 0..3 {
        fixtures::upload_text(&engine, actor, &format!("doc {i}")).await;
    }

    let removed = engine.delete_all_content(actor).await?;
    assert_eq!(removed, 3);
    assert!(engine.list_content(actor).await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn download_nonexistent_returns_error() -> Result {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let result = engine.download_content(actor, Uuid::new_v4()).await;
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn delete_nonexistent_returns_error() -> Result {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let result = engine.delete_content(actor, Uuid::new_v4()).await;
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn actors_cannot_see_each_others_content() -> Result {
    let (engine, _dir) = fixtures::engine();
    let actor_a = fixtures::actor();
    let actor_b = fixtures::actor();

    let id = fixtures::upload_text(&engine, actor_a, "secret").await;

    assert!(engine.list_content(actor_a).await?.contains(&id));
    assert!(!engine.list_content(actor_b).await?.contains(&id));
    Ok(())
}

#[tokio::test]
async fn actors_cannot_download_each_others_content() -> Result {
    let (engine, _dir) = fixtures::engine();
    let actor_a = fixtures::actor();
    let actor_b = fixtures::actor();

    let id = fixtures::upload_text(&engine, actor_a, "private").await;
    let result = engine.download_content(actor_b, id).await;
    assert!(result.is_err());
    Ok(())
}
