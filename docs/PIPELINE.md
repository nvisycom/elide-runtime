# The Runtime Pipeline

## Abstract

This paper describes the per-request workload the runtime drives over the
toolkit. The toolkit itself performs detection and redaction on one document at
a time; the runtime turns each call into a _run_ — a batched, durable, two-phase
lifecycle over many documents, governed by persisted policy, gated by reviewer
overrides, and tracked through explicit per-document states. What the toolkit
decides is in scope for [its own documentation series][elide-docs]. This paper
covers what the runtime adds: the lifecycle, the artifact, the governance
plumbing, and the failure semantics that come with operating the toolkit at
scale.

## 1. The unit of work

The runtime's unit of work is the _run_. A run is one submission of a batch of
files by one actor, processed against a fixed snapshot of policies and contexts
the actor has previously persisted. Files are not uploaded as part of the run;
they are uploaded once into the actor's file space and referenced by id.
Policies and contexts likewise are addressed by `(id, version)` and resolved
against the persisted snapshot at the moment the run starts. The run thereafter
is a closed object: every input it operates on, every governance choice that
applies to it, and every output it produces are linked together by the run's
identifier.

A run carries an explicit lifecycle state. It begins in _Analyzing_, transitions
to _AwaitingReview_ once detection has finished for every input that did not
fail or time out, becomes _Applied_ or _PartiallyApplied_ when reviewer
overrides have been resolved and the redactions written, and lands in _Failed_
if it could not produce any reviewable output. Each input document inside the
run has its own per-document state on the same axis: a document that timed out
during detection is not retried by the run's apply phase; a document whose apply
succeeded carries the output file id forward; a document whose apply failed
leaves its detection artifact intact for re-driving once the operator condition
is resolved.

The runtime fans the per-document work out under a caller-bounded concurrency
cap, with a per-document timeout. A single slow document does not stall its run;
it lands in _TimedOut_ and the rest of the batch settles. A single failed
document does not poison its run; it lands in _Failed_ with a reason and the
rest of the batch settles. The run header reports the aggregate at the end.

## 2. The two-phase split

The runtime separates detection from application into two distinct phases joined
by a durable artifact. The toolkit alone could fuse the two — its `Orchestrator`
exposes a single-call `anonymize` shorthand — but the runtime never uses that
path. Every run produces a detection artifact, persists it, and only then admits
reviewer overrides and runs apply against the persisted result.

```
    per-doc input file
          |
          v
+----------------------+         +-------------------+
|   ANALYZE PHASE      |  -----> |  RUN DOCUMENT     |
|   (per-document)     |         |  (persisted)      |
|   - decode bytes     |         |  - body modality  |
|   - run orchestrator |         |  - per-entity     |
|     against scope    |         |    findings       |
|   - record findings  |         |  - provenance     |
+----------------------+         |  - (optional)     |
                                 |    overrides      |
                                 +---------+---------+
                                           |
                               override    |
                               (optional)  |
                                           v
                               +-----------------------+
                               |   APPLY PHASE         |
                               |   (per-document)      |
                               |   - resolve policies  |
                               |     applicable to doc |
                               |   - layer overrides   |
                               |     ahead of policy   |
                               |   - run orchestrator  |
                               |     in apply mode     |
                               |   - persist output    |
                               |     as a new file     |
                               +-----------+-----------+
                                           |
                                           v
                                    output file
                                    + run document
                                      updated state
```

The artifact at the centre is the run document. It is per-input, durable, and
carries the toolkit's findings in the toolkit's own shape; the runtime persists
it unmodified. Three properties of this split deserve naming.

**Reviewability before mutation.** The analyze phase touches no original
content; it writes findings, not redactions. A reviewer can read the persisted
findings, override any per-entity decision, and trigger apply only after the
override set is acceptable. The toolkit's detection-and-application primitive is
not bypassed; it is just split across two callable points joined by storage.

**Replayability.** Because the detection artifact is durable, apply can be
re-run against the same artifact with a different policy snapshot or a different
override set. The cost of detection is paid once; the cost of trying alternative
redaction outcomes is paid as many times as the operator chooses.

**Failure localisation.** Detection failure cannot corrupt content; apply
failure cannot poison the detection artifact. A failure in either phase is
recoverable by retrying that phase against the artifact that survives the
boundary.

## 3. Files as the input/output interface

The runtime's only interface for content is the file. A caller uploads each
input file once, receives a file id, and from that point on references content
exclusively by id. Runs reference input files by id; reviewers download input
files by id; apply writes the redacted output as a new file and records its id
on the run document.

This choice keeps content out of the run boundary. A run header carries no bytes
— it carries references. The same input file may participate in many runs; a
redacted output file is itself a first- class object that survives its producing
run; the runtime can be restarted, queried, or migrated without re-uploading
content. The lineage between input and output is recorded on the output file's
metadata as a `RedactedFrom { run_id, source_file_id }` stamp, so that a
redacted artifact carries its provenance explicitly.

A consequence worth naming: cancelling or deleting a run does not cascade to the
files it referenced. Files are owned by the actor's file space and outlive any
individual run they participated in.

## 4. The artifact and override identity

The run document's persisted shape is the toolkit's `Report` extended with a
per-entity override slot. The runtime never invents an entity: every entry in
the artifact came from a toolkit recognizer. What the runtime adds is the seam
on which a reviewer can attach a decision without rewriting the artifact.

Override identity is the entity id the toolkit minted at detection time. Two
consequences:

- _Per-mention granularity is preserved._ A reviewer can override one occurrence
  of an entity while leaving another in the same document alone. The toolkit's
  view of which mentions are coreferent is a model output the runtime exposes;
  it is never a constraint the runtime imposes on review.
- _Overrides become diffable._ A set of overrides against a fixed detection
  artifact is fully determined. It can be stored, versioned, inspected, and
  replayed.

If a reviewer wishes to operate at a coarser granularity than a single entity,
the runtime permits that intent to be expressed as a batch of per-mention
overrides — but it does not allow the runtime itself to quietly fan a single
override out to multiple mentions. The explicit form is recoverable; the
implicit form is not.

## 5. Policy and context resolution

A run carries references to the policies and contexts the actor wants applied.
Each reference is an `(id, version)` pair. The runtime resolves every reference
at start time against the actor's persisted resources; a missing or revoked
resource fails the run before any detection runs, not silently mid-flight. The
resolved snapshot is stable for the run's lifetime: a policy edit after start
does not retroactively change a run already in progress, and re-running an apply
months later against the same artifact produces the same decisions if the same
`(id, version)` pairs still resolve.

Policies are scope-gated. A policy may declare a `applies_when` document
predicate; the runtime evaluates this predicate against each input document's
metadata + the run's per-call metadata, and skips policies whose predicate is
false for that document. The remaining policies for a document drive its apply
pass. The reviewer's overrides sit _in front of_ the policy chain — a per-entity
override fires before the per-label/per-tag policy rules. This is intentional:
the human reviewer is the senior decision-maker, and the audit reads in that
order.

Contexts are referenceable but the runtime does not impose interpretation. A
recognizer that wants context-dependent behaviour can consult the resolved
context set by id; the runtime delivers the set, not the interpretation. Context
content is owned by the actor; versioning protects long-lived runs from drift.

## 6. Per-document failure and cancellation

Both phases are long-running and both must be observable and recoverable. The
runtime's two-phase split makes the failure model explicit.

**Per-document failure.** A recognizer error, a timeout, or an apply operator
error fails one document, not its run. The document lands in _Failed_ or
_TimedOut_ with a reason recorded on its row. The run continues; aggregate state
at end-of-phase is _PartiallyApplied_ if any document failed apply, or _Applied_
if all succeeded. A failed document's artifact (if it reached one) is preserved
so the operator can investigate without re-running detection.

**Cancellation.** A run in _Analyzing_ or _AwaitingReview_ may be cancelled by
the operator. Cancellation is a header transition to _Failed_ with
`reason = "cancelled"`. The runtime today does not interrupt per-document
futures already in flight; those complete their current step and write into
per-document rows under a header that has moved on. Cooperative interruption —
threading a cancellation token through the per-document loops — is a future
slice; the audit currently records the header transition explicitly so a reader
can distinguish a cancelled run from a failed one.

**Delete.** Deleting a run removes the header and every per-document row owned
by the run. Input and output files are not cascaded — they are first-class
resources outside any one run. A delete on an active run is permitted;
per-document tasks that complete after the delete write into a keyspace that no
longer has a header and are reaped on the next list scan.

## 7. What this architecture buys

It is worth stating plainly what the runtime adds on top of the toolkit and what
it costs.

The runtime delivers _governance_: a place to put policies and contexts that
outlive any single call, a snapshot mechanism that freezes them per-run, and a
multi-tenant scope that keeps actors isolated. The toolkit cannot do any of
these, by deliberate design.

The runtime delivers _review_: a callable boundary at which a human can read
findings, override per-entity decisions, and trigger apply only when the
override set is acceptable. The toolkit's single-call shorthand is not used; the
runtime always exposes the seam.

The runtime delivers _operational visibility_: a per-document state machine that
records exactly what happened, an aggregate run state that summarises it, an
audit row per entity that connects machine finding to policy decision to
reviewer override. The toolkit emits its own per-entity provenance; the runtime
composes that provenance with the governance trail the toolkit does not see.

The price is the state the runtime must manage. Files, policies, contexts, runs,
per-document rows, output files — each is a first- class persisted object with
identity, lifecycle, and access scope. The runtime owns this management
explicitly rather than handing it off; that is the cost of being the layer
between an SDK consumer and the toolkit.

[elide-docs]: https://github.com/nvisycom/elide/tree/main/docs
