//! Run lifecycle, listing, and analytics tests.

mod fixtures;

use nvisy_engine::pipeline::RunFilter;
use uuid::Uuid;

#[tokio::test]
async fn list_runs_empty() -> Result<(), Box<dyn std::error::Error>> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let runs = engine.list_runs(actor, RunFilter { status: None }).await;
    assert!(runs.is_empty());
    Ok(())
}

#[tokio::test]
async fn get_nonexistent_run_returns_none() -> Result<(), Box<dyn std::error::Error>> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let run = engine.get_run(actor, Uuid::new_v4()).await;
    assert!(run.is_none());
    Ok(())
}

#[tokio::test]
async fn delete_nonexistent_run_returns_error() -> Result<(), Box<dyn std::error::Error>> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let result = engine.delete_run(actor, Uuid::new_v4()).await;
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn cancel_nonexistent_run_returns_error() -> Result<(), Box<dyn std::error::Error>> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let result = engine.cancel_run(actor, Uuid::new_v4()).await;
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn analytics_empty_engine() -> Result<(), Box<dyn std::error::Error>> {
    let (engine, _dir) = fixtures::engine();

    let snap = engine.snapshot().await;
    assert_eq!(snap.total_runs, 0);
    assert_eq!(snap.active_runs, 0);
    assert_eq!(snap.succeeded_runs, 0);
    assert_eq!(snap.failed_runs, 0);
    assert_eq!(snap.cancelled_runs, 0);
    assert_eq!(snap.distinct_actors, 0);
    assert!(snap.min_run_duration_ms.is_none());
    Ok(())
}

#[tokio::test]
async fn failed_run_appears_in_listing() -> Result<(), Box<dyn std::error::Error>> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    // An empty graph will fail compilation, creating a Failed run entry.
    let graph = nvisy_engine::graph::Graph::new(vec![], vec![]);
    let input = fixtures::engine_input(actor, graph);
    let result = engine.run(input).await;
    assert!(result.is_err());

    // The failed run should still appear in the listing.
    let runs = engine.list_runs(actor, RunFilter { status: None }).await;
    assert_eq!(runs.len(), 1);
    assert_eq!(
        runs[0].status,
        nvisy_engine::pipeline::RunStatus::Failed
    );
    Ok(())
}

#[tokio::test]
async fn failed_run_can_be_deleted() -> Result<(), Box<dyn std::error::Error>> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let graph = nvisy_engine::graph::Graph::new(vec![], vec![]);
    let input = fixtures::engine_input(actor, graph);
    let _ = engine.run(input).await;

    let runs = engine.list_runs(actor, RunFilter { status: None }).await;
    assert_eq!(runs.len(), 1);

    engine.delete_run(actor, runs[0].id).await?;
    let runs = engine.list_runs(actor, RunFilter { status: None }).await;
    assert!(runs.is_empty());
    Ok(())
}

#[tokio::test]
async fn delete_all_runs_clears_finished() -> Result<(), Box<dyn std::error::Error>> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    for _ in 0..2 {
        let graph = nvisy_engine::graph::Graph::new(vec![], vec![]);
        let _ = engine.run(fixtures::engine_input(actor, graph)).await;
    }

    let removed = engine.delete_all_runs(actor).await;
    assert_eq!(removed, 2);
    assert!(engine.list_runs(actor, RunFilter { status: None }).await.is_empty());
    Ok(())
}

#[tokio::test]
async fn runs_isolated_between_actors() -> Result<(), Box<dyn std::error::Error>> {
    let (engine, _dir) = fixtures::engine();
    let actor_a = fixtures::actor();
    let actor_b = fixtures::actor();

    let graph = nvisy_engine::graph::Graph::new(vec![], vec![]);
    let _ = engine.run(fixtures::engine_input(actor_a, graph)).await;

    let a_runs = engine.list_runs(actor_a, RunFilter { status: None }).await;
    let b_runs = engine.list_runs(actor_b, RunFilter { status: None }).await;
    assert_eq!(a_runs.len(), 1);
    assert!(b_runs.is_empty());
    Ok(())
}

#[tokio::test]
async fn analytics_reflects_failed_run() -> Result<(), Box<dyn std::error::Error>> {
    let (engine, _dir) = fixtures::engine();
    let actor = fixtures::actor();

    let graph = nvisy_engine::graph::Graph::new(vec![], vec![]);
    let _ = engine.run(fixtures::engine_input(actor, graph)).await;

    let snap = engine.snapshot().await;
    assert_eq!(snap.total_runs, 1);
    assert_eq!(snap.failed_runs, 1);
    assert_eq!(snap.distinct_actors, 1);
    Ok(())
}
