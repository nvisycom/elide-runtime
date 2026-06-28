# Nvisy Runtime: A Multi-Tenant Engine for Auditable Redaction

## Abstract

This document series describes the conceptual architecture of the Nvisy runtime,
a long-running service that turns the [`elide`][elide] PII
detection-and-redaction toolkit into a multi-tenant system with persisted
governance, durable per-document audit, and a reviewable two-phase lifecycle.
The toolkit decides _what is sensitive_ and _how to hide it_; the runtime
decides _whose data it is_, _which policy applies_, _whether a human gets to
look first_, _how long the evidence lives_, and _how to scale a single document
into a batched, cancellable, observable workload_.

The boundary between the two is deliberate. `elide` is a library: it has no
opinion on identity, persistence, scheduling, or review. The runtime is a
service: it owns multi-tenant storage, the policy and context vocabulary, the
file lifecycle, the audit log, and the operational surface. A redaction that
fires anywhere in the stack is jointly produced by a toolkit decision and a
runtime governance choice; the audit trail attributes each part to its origin.

Three concerns motivated separating the runtime from the toolkit. First,
_tenancy_: the toolkit's API is per-call and stateless, but a production
deployment has many customers whose policies, contexts, and content must not
commingle. Second, _durability of decisions_: in regulated domains, the
recogniser's findings, the policy that selected an action, and the reviewer's
overrides must all survive long enough to defend a redaction years after it ran.
Third, _review_: a reviewer who can intervene only after content has been
mutated is not a reviewer; the runtime splits detection from application so that
a human can read, edit, or reject findings before any byte is changed.

## Reader's guide

The remaining documents each take one slice of the runtime and develop it in
isolation. They are independent and may be read in any order, though the order
below moves from the workload outward to the deployment.

| Document                            | Subject                                                                                                                                                                                                                                                                             |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Pipeline](PIPELINE.md)             | The two-phase analyze-then-apply lifecycle the runtime layers on top of the toolkit: how a multi-document run is scheduled, how detection produces a reviewable artifact, how reviewer overrides slot in front of policy decisions at apply time.                                   |
| [Compliance](COMPLIANCE.md)         | The audit trail the runtime maintains around every redaction: what is recorded per entity, how policy and reviewer attributions are kept distinct, how the audit relates to the toolkit's per-entity provenance, and what compliance posture the runtime can and cannot underwrite. |
| [Infrastructure](INFRASTRUCTURE.md) | The runtime's deployment shape, persistence model, multi-tenant scoping, and operational primitives: a single process with embedded storage, sharded by actor, with explicit choices about what is and is not shared across instances.                                              |

## Glossary

The terms below are used throughout the series with the meanings given here.
Where a term already has a definition in the [elide glossary][elide-glossary]
the runtime uses it unchanged; runtime-specific terms appear here only.

- **Actor**: the unit of multi-tenant isolation. Every resource the runtime
  persists — policies, contexts, files, runs — is keyed by an actor id. Calls
  into the runtime carry an actor id; the runtime never crosses actor
  boundaries.
- **Engine**: the runtime's adapter over the toolkit. Owns the persistence layer
  and the per-document orchestrator construction; exposes the verbs that drive
  analyze and apply.
- **Policy**: a named, versioned governance document the caller persists ahead
  of time. Carries the rules and operators that decide what happens to each
  detected entity.
- **Context**: a named, versioned reference document the caller persists ahead
  of time. Carries the per-deployment vocabulary (locale, allowed countries,
  reference data) a policy or recognizer may consult.
- **Run**: one batched submission of input files for analyze + apply. Owns a
  UUIDv7 identifier, references the policies and contexts in scope, and tracks
  per-document state through the two phases.
- **Run document**: one input file inside a run. Carries the detection artifact
  for that file and, after apply, the redacted output file id.
- **File**: a content-addressed blob the runtime stores. Inputs are files;
  redacted outputs are files; the run is the trail that links one to the other.
- **Detection artifact**: the toolkit's per-document `Report`, persisted on the
  run document after analyze. Reviewer overrides edit this object; apply
  consumes it.
- **Override**: a per-entity reviewer decision recorded against an entity in the
  detection artifact. Takes precedence over the policy chain at apply time;
  appears explicitly in the audit so a reader can distinguish machine, policy,
  and human authorship.

[elide]: https://github.com/nvisycom/elide
[elide-glossary]: https://github.com/nvisycom/elide/blob/main/docs/README.md#glossary
