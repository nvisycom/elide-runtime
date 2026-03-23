use nvisy_core::content::{Content, ContentData, ContentSource};
use nvisy_engine::pipeline::Engine;
use nvisy_ontology::context::Context;
use uuid::Uuid;

#[test]
fn engine_construction() {
    let (engine, dir) = Engine::temp();
    assert_eq!(engine.data_dir(), dir.path());
}

#[tokio::test]
async fn content_crud() {
    let (engine, _dir) = Engine::temp();
    let actor_id = Uuid::new_v4();

    let data = ContentData::from_text(ContentSource::new(), "hello world");
    let content = Content::new(data);
    let id = engine.upload_content(actor_id, content).await.unwrap();

    let ids = engine.list_content(actor_id).await.unwrap();
    assert!(ids.contains(&id));

    let downloaded = engine.download_content(actor_id, id).await.unwrap();
    assert_eq!(downloaded.as_str().unwrap(), "hello world");

    engine.delete_content(actor_id, id).await.unwrap();
    let ids = engine.list_content(actor_id).await.unwrap();
    assert!(!ids.contains(&id));
}

#[tokio::test]
async fn context_crud() {
    let (engine, _dir) = Engine::temp();
    let actor_id = Uuid::new_v4();

    let context = Context::new("test-context", vec![]);
    let id = engine.upload_context(actor_id, context).await.unwrap();

    let ids = engine.list_contexts(actor_id).await.unwrap();
    assert!(ids.contains(&id));

    let downloaded = engine.download_context(actor_id, id).await.unwrap();
    assert_eq!(downloaded.name, "test-context");

    engine.delete_context(actor_id, id).await.unwrap();
    let ids = engine.list_contexts(actor_id).await.unwrap();
    assert!(!ids.contains(&id));
}

#[tokio::test]
async fn analytics_snapshot_empty() {
    let (engine, _dir) = Engine::temp();

    let snap = engine.snapshot().await;
    assert_eq!(snap.total_runs, 0);
    assert_eq!(snap.active_runs, 0);
    assert_eq!(snap.succeeded_runs, 0);
    assert_eq!(snap.failed_runs, 0);
    assert_eq!(snap.cancelled_runs, 0);
    assert_eq!(snap.distinct_actors, 0);
    assert_eq!(snap.total_entities_detected, 0);
    assert_eq!(snap.total_redactions_applied, 0);
    assert!(snap.min_run_duration_ms.is_none());
    assert!(snap.max_run_duration_ms.is_none());
    assert!(snap.avg_run_duration_ms.is_none());
}

#[tokio::test]
async fn run_listing_empty() {
    let (engine, _dir) = Engine::temp();
    let actor_id = Uuid::new_v4();

    let filter = nvisy_engine::pipeline::RunFilter { status: None };
    let runs = engine.list_runs(actor_id, filter).await;
    assert!(runs.is_empty());
}

#[tokio::test]
async fn delete_nonexistent_run_returns_error() {
    let (engine, _dir) = Engine::temp();
    let actor_id = Uuid::new_v4();

    let result = engine.delete_run(actor_id, Uuid::new_v4()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn content_isolation_between_actors() {
    let (engine, _dir) = Engine::temp();

    let actor_a = Uuid::new_v4();
    let actor_b = Uuid::new_v4();

    let data = ContentData::from_text(ContentSource::new(), "actor a content");
    let id = engine
        .upload_content(actor_a, Content::new(data))
        .await
        .unwrap();

    let a_ids = engine.list_content(actor_a).await.unwrap();
    assert!(a_ids.contains(&id));

    let b_ids = engine.list_content(actor_b).await.unwrap();
    assert!(!b_ids.contains(&id));
}

#[tokio::test]
async fn delete_all_content() {
    let (engine, _dir) = Engine::temp();
    let actor_id = Uuid::new_v4();

    for i in 0..3 {
        let data = ContentData::from_text(ContentSource::new(), format!("doc {i}"));
        engine
            .upload_content(actor_id, Content::new(data))
            .await
            .unwrap();
    }

    let ids = engine.list_content(actor_id).await.unwrap();
    assert_eq!(ids.len(), 3);

    let removed = engine.delete_all_content(actor_id).await.unwrap();
    assert_eq!(removed, 3);

    let ids = engine.list_content(actor_id).await.unwrap();
    assert!(ids.is_empty());
}

#[tokio::test]
async fn delete_all_contexts() {
    let (engine, _dir) = Engine::temp();
    let actor_id = Uuid::new_v4();

    for i in 0..2 {
        let ctx = Context::new(format!("ctx-{i}"), vec![]);
        engine.upload_context(actor_id, ctx).await.unwrap();
    }

    let ids = engine.list_contexts(actor_id).await.unwrap();
    assert_eq!(ids.len(), 2);

    let removed = engine.delete_all_contexts(actor_id).await.unwrap();
    assert_eq!(removed, 2);

    let ids = engine.list_contexts(actor_id).await.unwrap();
    assert!(ids.is_empty());
}
