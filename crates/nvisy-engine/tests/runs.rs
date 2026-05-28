//! Run lifecycle, listing, and analytics tests.

mod fixtures;

use nvisy_engine::pipeline::{RunFilter, RunOutcome, RunStatus};
use uuid::Uuid;

#[tokio::test]
async fn dry_run_returns_without_exporting() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine().await;
    let actor = fixtures::actor();
    let content_id = fixtures::upload_text(&engine, actor, "dry run test").await;

    let output = engine
        .run(fixtures::dry_run_input(actor, content_id))
        .await?;

    // Dry run should succeed and return a valid run_id.
    let snapshot = engine.get_run(actor, output.run_id).await.unwrap();
    assert!(matches!(snapshot.outcome, RunOutcome::Succeeded { .. }));

    // Dry run produces audits but no applied redactions.
    assert!(output.audits.iter().all(|a| a.entries_count() == 0));
    Ok(())
}

#[tokio::test]
async fn successful_run_has_succeeded_status() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine().await;
    let actor = fixtures::actor();
    let content_id = fixtures::upload_text(&engine, actor, "run test").await;

    let output = engine
        .run(fixtures::engine_input(actor, content_id))
        .await?;

    let snapshot = engine.get_run(actor, output.run_id).await.unwrap();
    assert!(matches!(snapshot.outcome, RunOutcome::Succeeded { .. }));
    Ok(())
}

#[tokio::test]
async fn successful_run_appears_in_listing() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine().await;
    let actor = fixtures::actor();
    let content_id = fixtures::upload_text(&engine, actor, "listing test").await;

    let output = engine
        .run(fixtures::engine_input(actor, content_id))
        .await?;

    let runs = engine.list_runs(actor, RunFilter { status: None }).await;
    assert!(runs.iter().any(|r| r.id == output.run_id));
    Ok(())
}

#[tokio::test]
async fn filter_runs_by_status() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine().await;
    let actor = fixtures::actor();
    let content_id = fixtures::upload_text(&engine, actor, "filter test").await;

    engine
        .run(fixtures::engine_input(actor, content_id))
        .await?;

    let succeeded = engine
        .list_runs(
            actor,
            RunFilter {
                status: Some(RunStatus::Succeeded),
            },
        )
        .await;
    assert_eq!(succeeded.len(), 1);

    let failed = engine
        .list_runs(
            actor,
            RunFilter {
                status: Some(RunStatus::Failed),
            },
        )
        .await;
    assert!(failed.is_empty());
    Ok(())
}

#[tokio::test]
async fn analytics_reflects_successful_run() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine().await;
    let actor = fixtures::actor();
    let content_id = fixtures::upload_text(&engine, actor, "analytics test").await;

    engine
        .run(fixtures::engine_input(actor, content_id))
        .await?;

    let snap = engine.snapshot().await;
    assert_eq!(snap.succeeded_runs, 1);
    assert_eq!(snap.distinct_actors, 1);
    assert!(snap.max_run_duration_ms.is_some());
    Ok(())
}

#[tokio::test]
async fn list_runs_empty() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine().await;
    let actor = fixtures::actor();

    let runs = engine.list_runs(actor, RunFilter { status: None }).await;
    assert!(runs.is_empty());
    Ok(())
}

#[tokio::test]
async fn get_nonexistent_run_returns_none() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine().await;
    let actor = fixtures::actor();

    let run = engine.get_run(actor, Uuid::new_v4()).await;
    assert!(run.is_none());
    Ok(())
}

#[tokio::test]
async fn delete_nonexistent_run_returns_error() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine().await;
    let actor = fixtures::actor();

    let result = engine.delete_run(actor, Uuid::new_v4()).await;
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn cancel_nonexistent_run_returns_error() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine().await;
    let actor = fixtures::actor();

    let result = engine.cancel_run(actor, Uuid::new_v4()).await;
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn analytics_empty_engine() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine().await;

    let snap = engine.snapshot().await;
    assert_eq!(snap.current_runs, 0);
    assert_eq!(snap.succeeded_runs, 0);
    assert_eq!(snap.failed_runs, 0);
    assert_eq!(snap.cancelled_runs, 0);
    assert_eq!(snap.distinct_actors, 0);
    assert!(snap.max_run_duration_ms.is_none());
    Ok(())
}

#[tokio::test]
async fn failed_run_appears_in_listing() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine().await;
    let actor = fixtures::actor();

    // A non-existent content ID fails at import, creating a Failed run entry.
    let input = fixtures::failing_input(actor);
    let result = engine.run(input).await;
    assert!(result.is_err());

    // The failed run should still appear in the listing.
    let runs = engine.list_runs(actor, RunFilter { status: None }).await;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, nvisy_engine::pipeline::RunStatus::Failed);
    Ok(())
}

#[tokio::test]
async fn failed_run_can_be_deleted() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine().await;
    let actor = fixtures::actor();

    let input = fixtures::failing_input(actor);
    let _ = engine.run(input).await;

    let runs = engine.list_runs(actor, RunFilter { status: None }).await;
    assert_eq!(runs.len(), 1);

    engine.delete_run(actor, runs[0].id).await?;
    let runs = engine.list_runs(actor, RunFilter { status: None }).await;
    assert!(runs.is_empty());
    Ok(())
}

#[tokio::test]
async fn delete_all_runs_clears_finished() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine().await;
    let actor = fixtures::actor();

    for _ in 0..2 {
        let _ = engine.run(fixtures::failing_input(actor)).await;
    }

    let removed = engine.delete_all_runs(actor).await;
    assert_eq!(removed, 2);
    assert!(
        engine
            .list_runs(actor, RunFilter { status: None })
            .await
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn runs_isolated_between_actors() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine().await;
    let actor_a = fixtures::actor();
    let actor_b = fixtures::other_actor();

    let _ = engine.run(fixtures::failing_input(actor_a)).await;

    let a_runs = engine.list_runs(actor_a, RunFilter { status: None }).await;
    let b_runs = engine.list_runs(actor_b, RunFilter { status: None }).await;
    assert_eq!(a_runs.len(), 1);
    assert!(b_runs.is_empty());
    Ok(())
}

#[tokio::test]
async fn analytics_reflects_failed_run() -> anyhow::Result<()> {
    let (engine, _dir) = fixtures::engine().await;
    let actor = fixtures::actor();

    let _ = engine.run(fixtures::failing_input(actor)).await;

    let snap = engine.snapshot().await;
    assert_eq!(snap.failed_runs, 1);
    assert_eq!(snap.distinct_actors, 1);
    Ok(())
}

#[tokio::test]
async fn detection_kinds_with_unconfigured_recognizer_fails_validation() -> anyhow::Result<()> {
    use nvisy_engine::detection::{Detection, RecognizerKind};

    let (engine, _dir) = fixtures::engine().await;
    let actor = fixtures::actor();
    let content_id = fixtures::upload_text(&engine, actor, "test").await;

    // Build an input that opts in to the LLM recognizer without
    // configuring `[recognizer.llm]` — the assembly step should
    // refuse.
    let mut input = fixtures::engine_input(actor, content_id);
    input.detection = Detection {
        kinds: vec![RecognizerKind::Llm],
        ..Default::default()
    };

    let result = engine.run(input).await;
    assert!(
        result.is_err(),
        "expected validation error for unconfigured recognizer"
    );
    Ok(())
}

#[tokio::test]
async fn empty_extraction_succeeds() -> anyhow::Result<()> {
    // Default Extraction config (no per-modality flags) is valid;
    // text content needs no extractor so the phase is a no-op.
    let (engine, _dir) = fixtures::engine().await;
    let actor = fixtures::actor();
    let content_id = fixtures::upload_text(&engine, actor, "no extraction needed").await;

    let input = fixtures::engine_input(actor, content_id);
    let output = engine.run(input).await?;

    let snapshot = engine.get_run(actor, output.run_id).await.unwrap();
    assert!(matches!(snapshot.outcome, RunOutcome::Succeeded { .. }));
    Ok(())
}
