# The Two-Phase Pipeline

## Abstract

Automated removal of sensitive content from heterogeneous documents
is conventionally treated as a single end-to-end transformation:
bytes in, redacted bytes out. This document describes an
alternative architecture in which the act of *identifying*
sensitive content and the act of *acting on* it are realised as
two distinct phases joined by an immutable, durable artifact. The
two phases have different cost structures, different failure
modes, and different review requirements. Coupling them, as a
conventional one-shot pipeline does, forces the worst
characteristics of each onto the other. Separating them yields a
system in which detection can be cached, redaction can be reviewed
before any byte is mutated, and failures localise to a single side
of the boundary.

## 1. Problem framing

A redaction system performs two qualitatively different workloads
back to back.

The first workload is *recognition*. For each piece of input
content, the system must enumerate every span, region, or interval
that is sensitive — typically by composing rule-based matchers with
statistical and generative models, often running expensive inference
over images, audio, or long text. Recognition is computationally
heavy, model-dependent, non-deterministic in the small, and
characterised by tail-latency rather than throughput. A single
document may pass through several recognisers; some may time out;
some may produce overlapping or contradictory findings; all of them
have measurable error rates.

The second workload is *application*. Given that a span of text or
a region of an image is sensitive, the system must execute a
transformation against it — masking, replacing, pseudonymising,
blurring, silencing — and re-emit the document in its original
format. This work is light on CPU but heavy on destructive writes:
it permanently mutates content in a way that cannot be undone
without re-deriving the document from the original. A wrong
recognition decision propagated into application is observable only
as missing or corrupted content.

Bundling these workloads into a single operation couples failures
that should fail independently. A transient recogniser timeout
aborts the redaction. A misconfigured operator destroys content
that recognition already correctly identified. A reviewer who
notices a probable false positive has no opportunity to intervene;
the bytes are already gone. Re-running on the same input re-incurs
the cost of recognition even when only the operator policy
changed. These problems are not specific to any particular
implementation; they follow from treating recognition and
application as a single atomic step.

## 2. The two-phase split

The architecture under discussion realises recognition and
application as two distinct phases. Each is invoked separately;
each produces a durable, addressable artifact; each can be observed
and managed in isolation.

```
                    +-----------------------+
   raw content ---> |   DETECTION PHASE     |
                    |  ingest -> extract    |
                    |  -> recognise -> dedup|
                    +-----------+-----------+
                                |
                                v
                  +----------------------------+
                  |   DETECTION ARTIFACT       |
                  |  (immutable, addressable)  |
                  |  - typed entity records    |
                  |  - per-entity provenance   |
                  |  - per-entity decision     |
                  |  - stable entity ids       |
                  +----+--------------+--------+
                       |              |
            overrides  |              |
            (optional) v              v  (replay, n times)
                  +-------------------------+
                  |   REDACTION PHASE       |
                  | resolve -> apply ->     |
                  |   encode -> audit       |
                  +-----+-------------+-----+
                        |             |
                        v             v
                 rewritten        audit
                 content          trail
```

The artifact at the centre is the load-bearing element. It is
typed, exhaustive over the recognised entity set, and never
mutated in place. Each entry within it carries enough provenance
to reconstruct, after the fact, which recognition strategy
produced the entry, what confidence was assigned to it, and which
decision rule selected the transformation that would be applied.

Three properties of this split deserve explicit attention.

**Replayability.** Because detection produces a durable artifact,
the same recognition output can drive multiple application passes
— one for previewing, another for archival, another for analytics —
without re-paying the cost of recognition. The artifact also
becomes a useful object in its own right: it can be compared
across recogniser versions, measured for precision and recall, and
shipped to a reviewer without exposing transformed content.

**Reviewability before mutation.** Because application is a
separate phase, the artifact can be inspected, modified, or
partially rejected before any byte of underlying content has been
touched. This is the difference between a compliance system that
can defend its decisions and one that has already irreversibly
acted on them. In regulated domains the former is a precondition
for deployment.

**Failure localisation.** Recogniser failure cannot corrupt
content; application failure cannot poison the recognition cache.
A failure in either phase is recoverable by retrying *that phase*
against a known-good artifact at the boundary.

The trade-off is honest: this architecture requires the system to
manage more state. The detection artifact is a first-class entity
with its own identity, lifecycle, retention policy, and access
controls.

## 3. Inside the detection phase

The detection phase is itself a small pipeline. Conceptually it
moves through four stages.

```
   raw bytes
       |
       v
  +----------+    +-----------+   +-------------+   +------------+
  | INGEST   | -> | EXTRACT   |-> | RECOGNISE   |-> | DEDUPLICATE|
  | (decode  |    | (OCR,     |   | (rule +     |   | (threshold,|
  |  to a    |    |  ASR,     |   |  statistic +|   |  overlap,  |
  |  typed   |    |  structure|   |  generative,|   |  fusion)   |
  |  handle) |    |  parsing) |   |  in parallel|   |            |
  +----------+    +-----------+   +-------------+   +------------+
                                                          |
                                                          v
                                                  detection artifact
```

**Ingestion** is format-specific decode. Heterogeneous input bytes
(documents, images, audio, structured records) become typed
in-memory handles. The handle's type encodes the modality;
downstream stages can rely on that typing rather than re-inspecting
raw bytes.

**Extraction** derives a scannable payload from modalities where
sensitivity exists in non-textual content but is most reliably
identified in textual form: optical character recognition for
images, transcription for audio, structure parsing for layout-rich
documents. Extraction does not erase the original modality; the
typed handle retains the underlying pixels or samples so that
later phases can act on them.

**Recognition** is pluralistic and parallel. Multiple recognition
strategies run side by side: deterministic patterns with checksum
validation; statistical models for named entity recognition over
text; generative models for context-sensitive identification;
vision models for face and document detection. Each strategy
operates over the appropriate payload (raw text, transcript,
extracted layout, pixels, samples). Each emits candidate entities
with a confidence score and a record of which strategy produced
the candidate.

**Deduplication** converges the candidate set into a stable entity
set. It is a layered pipeline, not a single step:

1. *Threshold filtering.* Candidates whose confidence falls
   below the configured threshold for their entity type are
   discarded. Different categories may admit different
   thresholds.
2. *Overlap resolution.* Where candidates overlap in
   modality-appropriate space (character ranges, pixel regions,
   time intervals), the system selects a winner according to a
   declared precedence — typically favouring higher confidence
   or more specific categorisation.
3. *Fusion.* Candidates from different strategies that
   describe the same underlying entity are merged. The merged
   entity inherits the union of its sources' provenance, so a
   later reader can see that, for example, the same span was
   independently flagged by a regex and by a model.

The output of detection is the immutable artifact described
above: a typed record of every surviving entity, with provenance,
confidence, and decision rationale. The artifact also stores
enough identifying information about the original content (by
reference, not by value) for the application phase to re-acquire
it.

## 4. Inside the redaction phase

The redaction phase consumes a detection artifact and produces two
outputs: rewritten content in the original format, and an audit
trail describing what happened. Like detection it decomposes into
conceptual stages.

```
   detection artifact
       +  optional overrides
       |
       v
  +--------------+   +-----------+   +-----------+   +----------+
  | RESOLVE      |-> | APPLY     |-> | ENCODE    |-> | AUDIT    |
  | (artifact +  |   | (per-     |   | (handle - |   | (per-    |
  |  overrides   |   |  entity   |   |  >        |   |  entity  |
  |  -> decision |   |  operator |   |  original |   |  outcome |
  |  set)        |   |  on typed |   |  format)  |   |  record) |
  |              |   |  handle)  |   |           |   |          |
  +--------------+   +-----------+   +-----------+   +----------+
                                                          |
                                                          v
                                                   rewritten content
                                                       +  audit
```

**Override resolution** combines the detection artifact with any
overrides supplied by a human reviewer or an external policy
system, producing a final decision set. Overrides express intent
at the granularity of a single entity: accept the recogniser's
choice as-is; reject it (so that the entity will not be
transformed); replace the chosen operator with a different one;
or insert an entity that recognition missed. Each form of
override is recorded in the eventual audit trail as a distinct
provenance value, so a reader can later tell which decisions
originated with the recognisers, which were affirmed by a
reviewer, and which were authored by a reviewer.

**Application** executes the chosen transformation for each
surviving entity against the typed handle that was re-derived
from the original content. Each operator is paired with its
modality: text spans are replaced in text handles, pixel regions
are mutated in image handles, sample intervals in audio handles,
cells in tabular handles. Operators are byte-level primitives
within their modality, but the dispatch is not byte-level: it is
done once, on modality, at the entry to this stage. (Section 5
treats this point in its own right.)

**Encoding** re-serialises the mutated handle back into the
document's original format. A redacted document remains the same
kind of artifact it was: a PDF stays a PDF, an image stays an
image, a tabular file stays tabular. Encoding is the inverse of
ingestion and is the point at which mutation becomes visible to
the outside world.

**Audit** records, per entity, what happened: whether the entity
was applied, suppressed by override, or failed to apply. The
audit is not a side effect; it is one of the two first-class
outputs of the phase. Section 6 develops its semantics.

## 5. Per-modality dispatch

Once the ingestion stage has produced a typed handle, the rest of
the pipeline avoids repeatedly asking "what kind of content is
this?". Dispatch on modality happens once, at the boundary into
each downstream stage, after which each branch operates
monomorphically — text operators see only text handles, image
operators see only image handles, and so on. The byte-level
mutation primitives at the leaves of the system are not modality-
agnostic; they are specific to their modality and statically
paired with it.

This is more than stylistic. Errors in modality pairing surface
early: an operator intended for one modality cannot be issued
against another; the mismatch is rejected before any byte is
touched, not silently absorbed into an unreachable code path.
Reasoning about each branch is local: a change to image redaction
does not require revisiting the text path. And the hot path
inside a branch is straight-line work on bytes of known shape,
without per-entity type interrogation.

The dispatch boundary is therefore a load-bearing architectural
element, not an incidental implementation detail. Adding a new
modality requires a new ingestion path, a new branch of the
dispatch, and a new family of operators — but no modification to
existing branches.

## 6. Audit trail semantics

Every entity that survives to the application phase produces an
audit entry. The entry records two things distinctly.

The first is the *decision*: which recogniser or rule produced
the entity, what confidence was attached, which operator the
recognition phase selected, and whether an override modified that
selection. The five decision provenances are *recogniser-only*,
*recogniser plus reviewer acceptance*, *recogniser plus reviewer
rejection*, *recogniser plus reviewer replacement*, and
*reviewer-authored insertion*. Each is recorded explicitly. A
reader of the audit trail can therefore distinguish, for any
redaction, whether it originated with a machine, was affirmed by a
human, was modified by a human, or was authored by a human.

The second is the *execution*: whether the chosen transformation
was actually applied, was suppressed (because an override said so),
or failed (because the operator itself errored). The decision and
the execution are independent: a decision can be made and not
executed; an execution can succeed or fail without changing the
decision that selected it.

The audit is the runtime's authoritative log of what happened. It
is append-only and tamper-evident; it accompanies the rewritten
content as a co-equal output of the phase.

## 7. Override target identity

A subtle problem appears as soon as detection and application are
separated: how does an override say which entity it refers to?

A reviewer cannot reference an entity by its content — two entities
may have the same surface text but represent different referents,
and entity sets are not stable across recognition runs. A reviewer
cannot reference an entity by its position alone, because
applying earlier overrides may shift the positions of later
entities. A reviewer cannot reference an entity by a recogniser-
internal model identifier, because the recogniser set may change.

The system's answer is that each surviving entity is assigned a
stable identifier at the moment it is written into the detection
artifact. The identifier is a per-mention value: distinct
occurrences of the same underlying entity receive distinct
identifiers. Overrides reference that identifier and only that
identifier.

Two consequences are worth naming.

- *Per-mention granularity is preserved.* A reviewer can reject one
  occurrence of an entity while accepting another. The
  recogniser's view of which mentions are coreferent is treated as
  a model output, not as a constraint on review.
- *Overrides become diffable.* A set of overrides against a fixed
  detection artifact is fully determined: it can be stored,
  versioned, replayed, and compared. There is no implicit
  dependency on the recognition run that produced the entities,
  beyond the artifact's own identity.

If a reviewer wishes to operate at the coreference-group level,
the system permits expressing that intent as a batch of per-
mention overrides — but it does not allow the system itself to
quietly fan a single override out to multiple mentions. The
explicit form is recoverable; the implicit form is not.

## 8. Cancellation and failure

Both phases are long-running and both must be cancellable.
Cancellation is cooperative: each phase checks at well-defined
boundaries whether it has been asked to stop, and additionally at
each yielded await point inside long-running inner loops.

For *detection*, the boundaries are the transitions between its
stages — between extraction and recognition, between recognition
and deduplication — together with the cancellation checks inside
extractor and recogniser loops. Cancellation observed during
detection produces no persisted artifact. The phase either
completes and writes a complete artifact, or it does not. Partial
artifacts are not durable, by construction; this is the
consistency point that makes detection cacheable and replayable.

For *redaction*, the boundaries are the transitions between
override resolution, application, encoding, and audit emission.
Cancellation observed inside application across multiple documents
is the harder case. Some documents may already have been encoded
and emitted before the cancellation lands; those bytes are not
rolled back, because the encoding step is the moment at which the
operation becomes externally visible and durable. The audit
records, per document, whether the document completed or was
aborted. A reviewer reading the audit after a cancellation sees
exactly which documents were redacted before the cancellation
landed and which were not. The system therefore prefers honest
partial visibility over the illusion of atomicity it cannot
provide.

Failure modes are handled analogously. A recogniser that errors on
one input does not abort detection on the others; the artifact
records which inputs succeeded. An operator that errors on one
entity does not abort application of the others; the audit records
which entities applied and which failed. Where the artifact
references content that has since been deleted (because retention
policy expired it between phases), the application phase fails
loudly rather than silently producing degraded output.

## 9. What this architecture buys

It is worth stating plainly what the two-phase split delivers and
what it costs, by contrast with two alternative architectures.

*Against a single-pipeline system* — one in which ingestion,
recognition, and application are fused into a single end-to-end
transformation — the two-phase split delivers:

- the ability to review recognition output before any byte is
  mutated;
- the ability to derive multiple application outputs from one
  recognition run, at the cost of one recognition and several
  cheap applications;
- explicit, independent failure semantics for the recognition and
  application halves;
- a recognition output that can be cached, replayed, evaluated,
  and compared as a first-class object.

*Against an unstructured detect-then-redact system* — one in which
recognition writes loose findings into some shared store and
application reads them back — the two-phase split delivers:

- a typed, addressable artifact that is the only contract between
  the phases, eliminating the ambient-state coupling that such
  systems typically develop;
- a stable per-entity identifier scheme that makes overrides
  diffable;
- a clear locus for retention and access control (the artifact
  itself), rather than a sprawl of intermediate state to be
  governed individually.

The price the system pays for these properties is the artifact
itself. Recognition output is now a durable, retained object with
its own identity and its own lifecycle. The system must manage
it: store it, retrieve it, expire it, access-control it, and
reason about its relationship to the originating content. That is
real engineering cost, not negligible, and not free in storage.

In the domains the system targets — where a single incorrect
redaction is a compliance event, where regulators may demand to
see the basis of a decision years after it was made, and where
the alternative is irreversible destruction of content based on
unreviewed model output — the cost is the right one to pay. The
two-phase split is not an optimisation. It is a design decision
about which failures the system is permitted to have.
