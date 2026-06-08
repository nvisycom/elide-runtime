# Pipeline architecture: detection ↔ redaction split

This document is the contract for the two-subsystem split. The
types in `detection/` and `redaction/` are designed against it;
the engine implementation lands against it in subsequent
commits. Anyone changing those files reads this first.

## Why two subsystems

Redaction without human review is a compliance liability in the
domains that matter most (healthcare, legal, finance). The old
unified `Engine::run` ran detection and apply as one atomic
operation: a recogniser false-positive deleted real content with
no opportunity to intervene. Splitting the pipeline lets a
reviewer inspect the policy chain's decisions before any bytes
move.

The split is also useful without a human: one detection can feed
multiple redaction passes (preview with `Mask`, commit with
`Fake`) without re-running NER.

## Surface

Two `Engine` methods. Two REST resources. No shared mode flag.

```
Engine::detect(input: DetectionInput) -> Result<Uuid, Error>
Engine::redact(input: RedactionInput) -> Result<Uuid, Error>
```

```
POST /detections                returns detection_id
GET  /detections                lists actor's detections
GET  /detections/{id}           returns DetectionSnapshot
DELETE /detections/{id}         removes from store
POST /detections/{id}/cancel    cooperative cancel

POST /redactions                body references detection_id
GET  /redactions                lists actor's redactions
GET  /redactions/{id}           returns RedactionSnapshot
DELETE /redactions/{id}         removes from store
POST /redactions/{id}/cancel    cooperative cancel
```

The old `Engine::run` and `/runs` resource are removed wholesale.

## Detection (immutable artifact)

A `DetectionResult` is the immutable outcome of one
`Engine::detect` call. Once produced, it is never edited. Each
`RedactionInput` references one detection by id; the same
detection can be referenced by many redactions.

Persistence: detection results live in the registry under a
dedicated keyspace (`detections_ks`), serialised as JSON. The
in-memory `DetectionState` mirror is volatile and lost on
process restart; reads after a restart hit the registry. The
write at end of `Engine::detect` is the consistency point — a
crash mid-detection produces no detection record, never a
half-written one.

The result carries the original `ImportFile` references. These
are by id (registry `ContentSource` UUIDs). When `Engine::redact`
runs, it re-opens the same content from the registry. The
content itself is governed by the registry's retention policy;
if it has been deleted between detect and redact, the redaction
fails with a clear error pointing to the missing import.

## Redaction (override-aware apply)

`Engine::redact` takes a `RedactionInput`:

```rust
struct RedactionInput {
    actor_id: Uuid,
    detection_id: Uuid,
    overrides: Vec<RedactionOverride>,
    exports: Vec<ExportFile>,
}
```

The engine:

1. Loads the detection. Returns `NotFound` if missing or
   actor-mismatched; `Validation` if the detection is not in a
   terminal state.
2. Calls `validate_overrides(&overrides)` to catch malformed
   input (duplicate targets, modality mismatch on `Add`).
3. Re-opens the imported content from the registry.
4. Re-builds the per-document trees from the imports (same
   importer as detection used).
5. Replays the detection's audit into each tree: entities the
   recognisers detected are placed back at their locations.
6. Applies overrides:
   - `Accept` marks the audit entry as `OverrideAccept` —
     provenance only; behaviour identical to no override.
   - `Reject` sets `Execution = Suppressed` and marks the audit
     entry as `OverrideReject`.
   - `Replace` substitutes the operator and marks
     `OverrideReplace`. The original chain pick is retained on
     the audit entry alongside the substituted operator.
   - `Add` synthesises an `Entity<M>` with a fresh UUID, places
     it into the tree at the override's `location`, runs the
     policy chain for it (unless `operator` is pinned), and
     marks `OverrideAdd`.
7. Runs the redaction phase + validation phase against the
   prepared tree.
8. Runs export.
9. Persists the `RedactionResult` to the registry under
   `redactions_ks`.

The audit trail records every override's provenance via the
`RedactionDecision` enum. A reviewer reading the final audit can
distinguish:

- `PolicyChain` — recogniser detected, policy chose, no human
  touched it.
- `OverrideAccept` — recogniser detected, policy chose, human
  reviewed and approved.
- `OverrideReject` — recogniser detected, human rejected.
- `OverrideReplace` — recogniser detected, policy chose, human
  swapped the operator.
- `OverrideAdd` — recogniser missed, human added.

These provenance values are non-negotiable for compliance: the
audit must defend every redaction in court if needed.

## Override target identity

Overrides reference an entity by its `Entity::id` (the
per-mention UUID), **not** its `entity_id` (the coreference
group identifier). Coreferent mentions are independent. To
reject every mention of a coreferent entity, submit one
`Reject` per mention.

The choice is deliberate: a reviewer must be able to reject one
mention while accepting another. The recogniser's coreference
linkage is a model output, not ground truth.

## Cancellation

Both `Engine::detect` and `Engine::redact` accept cooperative
cancellation through a per-task `CancellationToken` checked at:

- Every phase boundary (between extraction → detection →
  deduplication for detect; between override-application →
  redaction → validation → export for redact).
- Inside long-running recogniser / extractor / export loops, at
  every yielded await point.

Cancellation observed during detection produces a `Cancelled`
status and discards any in-progress audit. The persisted state
is absent.

Cancellation observed during redaction is the harder case. If
cancellation lands after the redaction phase has written bytes
to some documents but not others, the result is `PartialFailure`
with the audit recording which documents completed and which
were aborted. Bytes already written are not rolled back —
exports are append-only or codec-replacing, and once a codec has
emitted output the operation is durable. Reviewers reading the
audit see exactly which documents were redacted before the
cancel landed.

## Error taxonomy

| Situation                                            | `ErrorKind`     |
|------------------------------------------------------|-----------------|
| Detection / redaction does not exist                 | `NotFound`      |
| Detection / redaction belongs to a different actor   | `NotFound`*     |
| Detection exists but not yet terminal                | `Validation`    |
| Detection terminal but cancelled (no result)         | `Validation`    |
| Override duplicate target                            | `Validation`    |
| Override `Add` operator modality mismatches location | `Validation`    |
| Override `Replace`/`Add` operator missing for kind   | `Validation`    |
| Override targets `entity_id` not in detection's audit| `Validation`    |
| Cancellation observed                                | `Cancellation`  |
| Recogniser / extractor / codec error                 | `Runtime`       |
| Registry / fjall I/O error                           | `Internal`      |

\* Actor-scoping returns `NotFound` (not `Forbidden`) on
mismatch to avoid leaking existence to unauthorised callers.

## Test obligations

Before any of this ships to a user-facing endpoint, the test
matrix below must be green. The integration tests live in
`tests/` and exercise the engine + registry + (mocked) recogniser
end-to-end.

**Detection:**

- Happy path: text/image/audio/tabular import, every recogniser
  fires, terminal `Succeeded` with audit on disk.
- Partial failure: one of N imports fails to parse;
  `PartialFailure` with audits for the successful imports.
- Cancellation mid-detection: token fires during extraction,
  during detection, during deduplication. Each produces
  `Cancelled` and no on-disk artifact.
- Actor isolation: actor A cannot read actor B's detection.
- Restart durability: persist a detection, restart engine, read
  it back via `get_detection`.

**Redaction:**

- Happy path with empty overrides: applies policy chain to
  every entity; audit shows `PolicyChain` decisions.
- `Accept` overrides round-trip: audit shows `OverrideAccept`,
  output bytes identical to the no-override case.
- `Reject` override suppresses one entity; audit shows
  `OverrideReject` and `Execution::Suppressed`; output bytes
  reflect the suppression (entity not redacted).
- `Replace` override substitutes operator; audit shows
  `OverrideReplace`; output bytes show the new operator's
  result.
- `Add` override injects an entity; audit shows `OverrideAdd`
  with fresh UUID; output bytes show the new entity redacted.
- `Add` with pinned operator bypasses policy chain.
- `Add` without pinned operator runs policy chain (matches
  recogniser-detected behaviour).
- Modality validation: `Replace` for a text entity with an
  image operator rejected pre-engine via `validate_overrides`.
- Coreference: two mentions of one coreferent entity get
  independent overrides; rejecting one does not reject the
  other.
- Missing detection: `RedactionInput.detection_id` not found
  returns `NotFound`.
- Missing import: detection references a content_id since
  deleted; redaction fails clearly pointing at the missing
  import.
- Cancellation mid-redaction across N documents: documents
  completed before cancel keep their redactions; documents
  not yet processed are aborted; `PartialFailure` status.

**Cross-cutting:**

- Override referencing `entity_id` not present in the
  detection: `Validation` error, no partial redaction.
- Two concurrent redactions against the same detection succeed
  independently.
- Concurrent override application within one redaction is
  ordered: overrides are applied serially per document so the
  audit is deterministic.

## Out of scope (this PR)

- Multi-detection redaction (one redaction targeting two
  detections at once). Not a real use case yet.
- Override-only redaction without re-importing (skip extraction
  + detection on restart). Optimisation; correctness first.
- Streaming detection results to the caller as documents
  complete. Today: caller polls `get_detection`.
- Detection retention separate from audit retention. Detection
  results inherit the audit's retention policy today.
