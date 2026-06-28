# Deployment Architecture

## Abstract

This paper describes the deployment shape of the Nvisy runtime and the boundary
between concerns the runtime owns and concerns the surrounding deployment must
provide. It is intended for platform engineers evaluating the system for
production use. The treatment is conceptual: it addresses the structure of the
deployment unit, the persistence model, the multi-tenant scoping, and the
operational primitives the runtime exposes. Specifics of any particular hosting
environment are out of scope.

## 1. Deployment philosophy

The runtime ships as a single self-contained binary with embedded storage. It
does not require a separate database, message broker, or coordination service. A
working installation is a process and a data directory.

This is a deliberate choice rather than an incidental one. Privacy- sensitive
systems benefit from minimising the number of components that handle, observe,
or could be compelled to surrender sensitive material. Every additional
dependency is a potential side channel: a log line in a database server, a
message queue that retains payloads briefly, a coordination service that
participates in routing decisions. Each of these expands the trust surface that
must be reasoned about, audited, and defended.

The runtime accepts a corresponding constraint: it trades horizontal scale-out
_inside_ the runtime itself for trust-surface simplicity. A deployment that
needs to spread load across multiple machines does so by running multiple
independent runtime instances, each owning its own state, and sharding work at
the layer above. The runtime does not coordinate among instances.

This trade-off is suitable for the class of workloads the runtime is designed
for. It is not suitable for workloads that require shared mutable state across
machines.

## 2. The deployment unit

A deployment unit is one process. The process exposes an HTTP interface for
programmatic access and reads from and writes to a local data directory for
persistent state. There is no separate worker tier, no auxiliary daemon, and no
sidecar requirement.

Concurrency within the unit is asynchronous: many requests are in flight inside
a single process, scheduled cooperatively on a Tokio runtime. Per-run document
fan-out is bounded by a caller-supplied concurrency cap with per-document
timeouts; the runtime exposes the cap rather than choosing one for the operator.
Concurrency across processes is not provided. A deployment that requires
multiple instances must arrange for inputs to be partitioned externally, and
each instance owns its partition exclusively.

The process is restartable without ceremony. State that must survive a restart
lives in the data directory; everything else is reconstructed at startup. There
is no warm cache the runtime maintains in process memory; a restart is
operationally equivalent to a fresh start against the same data directory.

## 3. The persistence model

The runtime persists six categories of state, all in a single embedded LSM-tree
store ([`fjall`][fjall]), each in its own keyspace keyed by the multi-tenant
scope.

**Policies.** Versioned governance documents the actor uploads. Keyed
`(actor_id, policy_id, version)`. Immutable per key. The runtime keeps every
version that has been written; older versions remain queryable as long as a run
header still references them.

**Contexts.** Versioned reference documents the actor uploads. Identical shape
to policies; same `(actor_id, context_id, version)` key. Mirror module, mirror
lifecycle.

**Files.** Two paired keyspaces: a small metadata keyspace
(`(actor_id, file_id) → FileMetadata`) and a blob-separated content keyspace
(`(actor_id, file_id) → bytes`). The split lets list and metadata reads be cheap
without paying the cost of loading bytes; bytes are fetched on demand by id.

**Run headers.** One header per run, keyed `(actor_id, run_id)`. The header
tracks lifecycle state, the policy and context snapshot, the per-doc id list,
the analyzer plan, and the concurrency cap.

**Run documents.** One row per input document inside a run, keyed
`(actor_id, run_id, doc_id)`. Carries the detection artifact (the per-modality
entity records + reviewer overrides), the input file id, the output file id
(once apply lands), and per-document state.

Every keyspace key is prefix-scannable on the actor id. List operations within a
tenant are O(actor's objects), not O(global), by construction. Bytes (inputs and
outputs) live exclusively in the files keyspace; no run keyspace carries
content.

## 4. Multi-tenant isolation

Every persisted object is scoped to an _actor_ — the multi-tenant unit of
isolation. The actor id is the leading component of every storage key, and the
runtime never reads or writes a key whose actor id was not supplied by the
caller of the current request.

Isolation is structural. The runtime's storage API takes an actor id as the
first parameter of every method; the keyspace layout guarantees that a scan
within an actor's prefix yields only that actor's objects. There is no per-call
permission check that could be forgotten or bypassed because the call shape
would not allow it.

What lives outside the runtime is the question of _whose actor id the caller is
allowed to claim_. The runtime does not authenticate the caller and does not
authorise the actor id assertion. The HTTP surface accepts the actor id from an
upstream component — an API gateway, an IAM integration, a session middleware —
that is responsible for binding a verified principal to the actor id it may
operate against. The runtime trusts that binding; the deployment is responsible
for not breaking it.

## 5. External dependencies

The runtime acquires no service dependencies at runtime. It loads its
configuration from a file at startup, opens its data directory, binds its HTTP
listener, and serves requests.

Optional dependencies appear only when the operator opts into features that need
them. Two examples in the current shape:

- Recognition backends. The default deployment runs with in-process pattern and
  dictionary recognizers and no NER or LLM. An operator who configures a NER
  backend brings up an external inference service (e.g. a BentoML host); the
  runtime calls it over HTTP. Without that configuration the runtime stays
  single-process and network-isolated.
- Observability. The runtime emits structured logs and tracing spans. A
  deployment that wants those exported to an external collector runs the
  collector; the runtime does not host one.

No optional dependency is required for the runtime to start, serve requests, or
persist state. The minimum deployment is one process and one data directory.

## 6. Operational primitives

The runtime exposes a small number of operational primitives the deployment is
expected to drive.

**Health checks.** A liveness probe and a readiness probe over HTTP. The
readiness probe reports per-component status — storage, codec registry, optional
backends — so a load balancer can route around an instance whose persistence
layer is unavailable.

**Graceful shutdown.** The HTTP layer drains in-flight requests on SIGTERM,
flushes the persistence layer to disk, and exits. The data directory after a
graceful shutdown is consistent and immediately re-openable.

**Backup.** The data directory is the unit of backup. A consistent snapshot is
taken by stopping the process (or pausing writes through the operational
interface) and copying the directory. The runtime does not ship a logical backup
tool; a deployment that wants one runs it at the HTTP layer.

**Delete.** Per-resource delete is exposed for every persisted category. Deletes
are not cascaded across categories: deleting a run does not delete the files it
referenced; deleting a file does not delete the runs that referenced it. This is
intentional — files are first-class resources, and the consequence of a stale
reference is a clean lookup failure, not a silent corruption.

**Cancellation.** A run in progress can be cancelled by the operator.
Cancellation is a header-level transition; in-flight per-document work continues
until its current step completes and writes its outcome into a row under a
header that has moved on. The audit records the cancellation explicitly so the
partial visibility is honest rather than hidden.

## 7. Scaling shape

The runtime scales by sharding actors across independent instances. Each
instance owns a data directory; each actor's data lives in exactly one instance;
the layer above is responsible for routing an actor's request to its owning
instance.

Within a single instance, throughput is bounded by the single-process I/O
ceiling of the embedded store and the single-process CPU ceiling of the
recognition pipeline. Both are adequate for the workloads the runtime targets —
bounded actor counts, batched run submissions, document-level concurrency in the
double digits — and inadequate for workloads that need shared mutable state
across instances or fan-out factors beyond what one process can sustain. The
runtime does not pretend otherwise. A deployment that outgrows a single
instance's envelope shards; a deployment whose access pattern cannot be sharded
by actor is using the wrong tool.

## 8. What the deployment must provide

The runtime provides the workload; the deployment provides the operational
envelope. Specifically:

- _Identity._ Authentication of the caller and binding to an actor id. The
  runtime trusts the actor id it receives; the deployment must ensure that trust
  is sound.
- _Authorisation._ Per-endpoint access control, rate limits, request shaping.
  The runtime accepts whatever the layer above forwards.
- _Transport security._ TLS termination, client certificate validation, anything
  that lives between the network and the runtime's HTTP interface.
- _Retention scheduling._ The runtime exposes delete hooks per category; the
  deployment decides the schedule.
- _Backup, restore, and disaster recovery._ The data directory is the persistent
  unit; the deployment owns its copy schedule and recovery procedure.
- _Observability transport._ The runtime emits logs and spans; the deployment
  runs the collector that ingests them.
- _Sharding._ If the workload outgrows one instance, the deployment routes
  actors across instances; the runtime offers no cross-instance coordination.

The line between runtime and deployment is the line between _what the workload
requires_ and _what every workload requires_. The runtime owns the former
exclusively and leaves the latter to the substrate that operates it.

[fjall]: https://github.com/fjall-rs/fjall
