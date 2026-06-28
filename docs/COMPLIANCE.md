# Compliance and Audit

## Abstract

This paper describes the compliance posture the Nvisy runtime
underwrites, the audit trail it produces, and the boundary between
what the runtime is responsible for and what it deliberately leaves to
layers above and below it. The intended reader is a compliance
engineer, internal auditor, or external assessor evaluating whether
the runtime is suitable for inclusion in a controlled environment.
The treatment is conceptual: it addresses the model that makes
the runtime auditable, not the operational controls a deployment must
wrap around it.

## 1. The runtime as a building block

Compliance is a sociotechnical concern, not a software feature. No
runtime can deliver "compliance with GDPR" or "SOC 2 certification" on
its own; only an organisation can, through audit-defensible processes
that span legal review, organisational policy, operational controls,
personnel training, and a body of evidence far larger than any single
piece of software. A redaction engine cannot certify a hospital, a
bank, or a data processor; it can only supply one of the load-bearing
components such a certification depends on.

What the runtime delivers is an auditable detection-and-redaction
pipeline: a mechanism for identifying sensitive entities in
unstructured content, deciding what to do with each one under a
stated policy, executing those decisions, and recording every step in
a form that a downstream auditor can read, replay, and challenge. The
pipeline is a building block. The compliance posture is the building.

## 2. The composite audit trail

Every redaction the runtime applies has two authors: the [`elide`][elide]
toolkit, which identified the entity and selected the operator that
hides it; and the runtime's governance layer, which decided whether the
toolkit's selection should fire at all and under whose authority. The
audit trail reflects that composition.

The toolkit supplies *per-entity provenance*: which recognizer
produced the entity, what evidence was cited, what confidence was
assigned, which operator class executed, what the leak profile of the
operator was. This information is carried inside the entity itself
through detection and into apply, and is preserved verbatim in the
detection artifact the runtime persists.

The runtime supplies *attribution*: which policy and rule was in
scope, whether a reviewer override took precedence, and the actor
context the whole pipeline ran under. Each redaction's audit entry
links a toolkit-side provenance to a runtime-side attribution, so a
reader can answer both "what did the model see?" and "under what
authority was this redaction applied?".

Every redaction record carries five dimensions, distributed across
the two authors:

**Identity.** A stable per-entity identifier, minted by the toolkit
at detection time and preserved through apply. Reviewer overrides
reference exactly this identifier; the audit row keys on it.

**Provenance.** Which recognizers contributed, what evidence each
one provided, what confidence each one assigned. Authored by the
toolkit, persisted verbatim.

**Decision.** Which policy rule matched and which operator was
selected; whether a reviewer override modified that selection.
Authored jointly: the toolkit knows which operator class fired, the
runtime knows which policy and reviewer chose it.

**Execution.** Whether the chosen operator applied, was suppressed
by override, or failed. Decision and execution are independent: a
decision can be made and not executed; an execution can succeed or
fail without changing the decision that selected it.

**Attribution.** The actor under whose scope the run ran, the
policy id and version that authorised the rule, and — when a
reviewer overrode — the override's authorship. The runtime stamps
this onto every redaction's `Attribution` slot; the toolkit treats
the slot as opaque and carries it along.

## 3. Two-phase decisions in the trail

The two-phase pipeline (detection then application, joined by a
durable artifact) is itself a compliance affordance. The audit
distinguishes four decision provenances explicitly:

- *Recognizer-only.* A recognizer asserted the entity; a policy rule
  selected an operator; no reviewer intervention. The most common
  case.
- *Recognizer + reviewer acceptance.* A recognizer asserted the
  entity; the reviewer was shown the finding and did not override.
  Recorded distinctly from *recognizer-only* when the deployment runs
  a mandatory-review workflow.
- *Recognizer + reviewer override.* A recognizer asserted the entity;
  the reviewer replaced the policy's decision with a per-entity
  override. The override's action sits ahead of the policy chain at
  apply.
- *Reviewer-authored insertion.* The recognizer missed an entity; the
  reviewer inserted an override that creates one. (This path is
  supported via the override mechanism even though no recognizer
  produced the underlying finding.)

The audit can therefore answer, for any redaction the runtime applied
or suppressed, whether it originated with a machine, was affirmed by
a human, was modified by a human, or was authored by a human. That
distinction is the load-bearing one for regulators who care about who
is accountable for a given redaction.

## 4. The retention boundary

The runtime persists three categories of long-lived material, each
with different compliance implications.

**Governance resources.** Policies and contexts are versioned and
immutable per `(id, version)`. The runtime keeps every version that
has been written, because runs may reference older versions and an
audit may need to reconstruct a decision under the policy that
authorised it at the time. Operators delete versions explicitly; the
runtime does not garbage-collect.

**Content.** Input files and redacted output files are stored as
content-addressed blobs. The runtime exposes a per-file delete; it
does not impose a retention horizon. Deployments that must enforce a
retention horizon do so by issuing scheduled deletes against the file
API.

**Run state.** Run headers and per-document detection artifacts are
the audit trail itself. They are kept as long as the deployment
keeps them; the runtime offers a per-run delete that cascades to the
per-document rows but, importantly, does not cascade to the input or
output files. A run that has been deleted leaves redacted output
files intact, with their `RedactedFrom` lineage pointing at a run id
that no longer resolves. This is deliberate: the redacted output is
the artifact a downstream system received, and the audit trail's
absence is itself a recorded fact (a redacted file whose run id
fails to resolve identifies the deletion event by its consequence).

The runtime does not impose a retention policy on any of these. It
exposes the lifecycle hooks; the deployment decides the schedule.
Retention is a governance choice, and a runtime that made it
automatically would be making policy on behalf of its operator.

## 5. Multi-tenant isolation

Every persisted object is scoped to an *actor*: policies, contexts,
files, runs, per-document rows. The actor id is supplied by the
caller on every API call and is the first component of every
storage key. The runtime never crosses actor boundaries: an actor
cannot read another actor's policies, reference another actor's
files, or observe the existence of another actor's runs.

Isolation is structural, not advisory. The keyspace layout
guarantees that a scan within an actor's prefix yields only that
actor's objects; there is no per-call permission check that could be
forgotten or bypassed because the call shape would not allow it.

What the runtime does not provide is *authorisation of the actor
id itself*. The actor id is an opaque scoping token; whatever
authenticates the caller and binds it to an actor lives in the layer
above the runtime — the API gateway, the IAM integration, the
session middleware. The runtime accepts whatever actor id its caller
asserts; the deployment must ensure that assertion is itself
trustworthy.

## 6. What the runtime does and does not underwrite

The runtime underwrites:

- A composite per-redaction audit that links toolkit-side recognition
  evidence to runtime-side policy attribution.
- A two-phase pipeline that admits human review of every recognition
  finding before any byte is mutated.
- A stable per-entity identifier scheme that makes overrides
  diffable, versionable, and replayable.
- Multi-tenant isolation enforced by storage layout, not by per-call
  checks.
- Versioned, immutable governance resources whose snapshots survive
  the runs that referenced them.
- Explicit lifecycle hooks for delete and cancel, with honest
  partial-visibility semantics where atomicity cannot be promised.

The runtime does not underwrite:

- Authentication of the actor that calls in.
- Authorisation of which actors may invoke which endpoints.
- The truthfulness of recognizer findings. Recogniser quality is a
  property of the recognizer; the runtime persists the output and
  the per-recognizer provenance, but it cannot certify accuracy.
- The interpretation of policies. The runtime executes a policy that
  a deployment authored; it cannot verify that the policy correctly
  encodes a regulatory obligation.
- A retention schedule. Retention is governance, not infrastructure;
  the runtime exposes the hooks the deployment must drive.
- A cryptographic seal on the audit trail. The trail is structured,
  append-as-state-transitions, and queryable; tamper-evidence is a
  property of the storage substrate and operator-level controls
  around it, not of the runtime itself.

The boundary between underwritten and not-underwritten is the
boundary between what the runtime can guarantee unilaterally and
what requires participation from the surrounding deployment. A
compliance posture is an integration result; the runtime is one of
its inputs.

[elide]: https://github.com/nvisycom/elide
