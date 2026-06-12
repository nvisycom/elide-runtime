# Deployment Architecture

## Abstract

This paper describes the deployment shape of the runtime and the boundary
between concerns the runtime owns and concerns the surrounding deployment must
provide. It is intended for platform engineers evaluating the system for
production use. The treatment is conceptual: it addresses the structure of the
artifact, the persistence model, the external dependencies the runtime may
acquire, and the operational primitives it exposes. Specifics of any particular
hosting environment are out of scope.

## 1. Deployment Philosophy

The runtime ships as a single self-contained binary with embedded storage. It
does not require a separate database, message broker, or coordination service.
A working installation is a process and a data directory.

This is a deliberate choice rather than an incidental one. Privacy-sensitive
systems benefit from minimizing the number of components that handle, observe,
or could be compelled to surrender sensitive material. Every additional
dependency is a potential side channel: a log line in a database server, a
message queue that retains payloads briefly, a coordination service that
participates in routing decisions. Each of these expands the trust surface that
must be reasoned about, audited, and defended.

The runtime accepts a corresponding constraint: it trades horizontal scale-out
inside the runtime itself for trust-surface simplicity. A deployment that needs
to spread load across multiple machines does so by running multiple independent
runtime instances, each owning its own state, and sharding work at the
application layer. The runtime does not coordinate among instances.

This trade-off is suitable for the class of workloads the runtime is designed
for. It is not suitable for workloads that require shared mutable state across
machines.

## 2. Process Model

A deployment unit is one process. The process exposes a network interface for
programmatic access and reads from and writes to a local data directory for
persistent state. There is no separate worker tier, no auxiliary daemon, and no
sidecar requirement.

Concurrency within the runtime is asynchronous: many requests are in flight
inside a single process, scheduled cooperatively. Concurrency across processes
is not provided by the runtime. A deployment that requires multiple instances
must arrange for inputs to be partitioned externally, and each instance owns its
partition exclusively.

The process is restartable without ceremony. State that must survive a restart
lives in the data directory; everything else is reconstructed at startup.

## 3. State and Persistence

The runtime persists three categories of state.

**Content.** The raw bytes of ingested material, retrievable by a stable
identifier. Content is the largest category by volume and the most sensitive by
nature.

**Decisions.** The detection and redaction artifacts produced by the processing
pipeline: what was found, where, with what confidence, and what transformation
was applied. Decisions are the audit substrate of the system.

**Operational metadata.** Policies, annotations, retention configurations, and
similar control-plane state. This category is small in volume but
high-consequence: it governs how content and decisions are produced and how
long they are kept.

All persistent state lives in an embedded ordered key-value store colocated
with the process. The three categories occupy separate logical keyspaces. This
separation is load-bearing: retention policies, encryption choices, and access
patterns differ between categories, and a shared keyspace would entangle them.

The store assumes exclusive ownership of its data directory. Running two
processes against the same data directory is unsupported and will produce
undefined behavior. Backup procedures must respect this invariant: a consistent
backup is one taken against a quiesced process or via a snapshot mechanism that
the store explicitly supports.

## 4. External Dependencies

Most of the runtime is self-contained. Two recognition modes optionally extend
the system with capabilities that exceed what a single process can reasonably
host, and these are treated as deployment-side dependencies rather than runtime
internals.

**Named-entity recognition.** An offline model served by an external inference
server. The server is a separate process, typically on a separate machine with
hardware suited to model inference. The runtime communicates with it over the
network and tolerates its absence.

**Generative recognition.** A large language model accessed over an API. This
may be a hosted service or a self-operated endpoint. The runtime treats the
endpoint as opaque: it sends prompts and consumes responses.

Neither dependency is required for the runtime to function. Pattern-based
recognition operates without either. Deployments that require named-entity or
generative recognition provision the corresponding services and configure the
runtime to use them. The runtime adapts to whatever is available; the absence
of an optional dependency disables the corresponding capability but does not
prevent startup.

This pattern preserves the single-binary philosophy: the runtime itself does
not embed model weights or inference engines, and the operational complexity
of running such services remains where it belongs, with the platform team.

## 5. Observability

The runtime emits structured tracing throughout its execution. Trace targets
follow a uniform hierarchical convention that mirrors the system's internal
structure, so deployments can filter traces at any level of granularity: an
entire subsystem, a specific stage of the pipeline, or a single operation.

The runtime also exposes a health endpoint that aggregates the status of each
subsystem the deployment should care about: the embedded store, the network
interface, and any configured inference dependencies. The endpoint reports
which subsystems are healthy, which are degraded, and which are unreachable. A
deployment's orchestration layer consumes this endpoint to drive readiness and
liveness checks.

Beyond these primitives, observability is a deployment concern. Log
aggregation, metric collection, dashboarding, alerting, and incident routing
are provided by the surrounding platform. The runtime makes no assumptions
about which tools are used and emits its tracing in a form that standard
collectors can consume.

## 6. At-Rest Encryption

Encryption of stored content is optional and entirely deployment-driven. The
runtime exposes a contract for an opaque key provider; a deployment that
requires at-rest encryption implements that contract using whatever key
management system, hardware security module, or rotation policy is appropriate.
The runtime itself does not manufacture, store, or rotate keys.

The runtime does not retain long-lived keys in process memory. A key is held
only for the duration of a single request, then released. This limits the
blast radius of a process compromise and aligns the runtime's behavior with
the principle that secret material lives in the deployment's key custody
infrastructure, not in the application that consumes it.

Deployments that do not require at-rest encryption omit the key provider
entirely; content is stored in cleartext within the data directory, and the
deployment relies on filesystem-level protections instead.

## 7. Compression

Compression of stored content is similarly optional. When enabled, content is
compressed before persistence and decompressed transparently when the pipeline
reads it back. The choice of algorithm is deployment-configurable; the runtime
treats the codec as a pluggable component.

Compression is a cost optimization, not a security feature. It interacts with
at-rest encryption in the usual order: compress first, then encrypt.

## 8. The Runtime / Deployment Boundary

This section is the conceptual core of the paper. Privacy systems fail in
production most often not because of defects in the runtime but because the
boundary between runtime and deployment was implicit, and concerns landed on
the wrong side of an unspoken line. The boundary is therefore stated
explicitly.

| Concern | Owned by Runtime | Owned by Deployment |
|---|---|---|
| Detection, redaction, audit | yes | |
| Format decoding, encoding, lifting | yes | |
| Registries of recognizers, operators, policies | yes | |
| Embedded persistence layer | yes | |
| Network interface for programmatic access | yes | |
| Structured tracing and health reporting | yes | |
| Authentication of callers | | yes |
| Authorization of operations | | yes |
| Transport encryption (TLS termination) | | yes |
| Backup and disaster recovery | | yes |
| Multi-tenant isolation enforcement | | yes |
| Log shipping and metrics aggregation | | yes |
| Provisioning of inference services | | yes |
| Key management and rotation | | yes |
| Quota enforcement and rate limiting | | yes |
| Capacity planning and horizontal sharding | | yes |

The runtime exposes actor-scoped namespaces in its persistence and processing
layers, so the deployment can pass an identity through and have the runtime
keep state separated accordingly. The runtime does not verify the identity; it
trusts the network edge to have done so. A deployment that fails to
authenticate at the edge effectively grants every caller every identity.

This division is the load-bearing assumption of the architecture. Mixing the
two sides — for instance, expecting the runtime to enforce identity, or
expecting the deployment to manage detection policies — produces systems that
are neither secure nor maintainable.

## 9. Scaling

The runtime scales vertically. Additional cores and memory allow more
concurrent requests within a single process. The asynchronous execution model
makes this effective up to the limits of the host.

The runtime does not scale horizontally in the conventional sense. The
embedded store assumes single-writer semantics: exactly one process owns a
given data directory at a time. A deployment that exceeds the capacity of a
single instance runs multiple instances, each with its own data directory, and
partitions inputs at the application layer such that any given identity is
served by a single instance.

This is a deliberate trade-off. A single-writer architecture eliminates entire
classes of concurrency bugs — distributed deadlocks, partial-failure
reconciliation, split-brain conditions — that would otherwise be present in
the data path of a privacy system. The cost is that scale-out requires
deployment-level sharding logic. The benefit is predictability: the runtime
behaves the same way under load as it does in isolation, and there is no
hidden coordination protocol that can degrade in ways the deployment cannot
observe.

For workloads where this trade-off is unacceptable, the runtime is the wrong
choice and a different system should be selected.

## 10. What the Architecture Does Not Provide

In the interest of accuracy, the following capabilities are explicitly out of
scope. A deployment that needs them must build them around the runtime.

- Cluster coordination among multiple runtime instances.
- Multi-region failover or active-active replication.
- Replicated storage at the persistence layer.
- Service mesh integration beyond what a standard network client provides.
- Pre-built container images, orchestrator manifests, or installation
  automation tailored to specific platforms.
- A user interface; all interaction is programmatic.
- Identity provisioning, directory integration, or federated authentication.

These omissions are not oversights. Each represents a category of
functionality that varies substantially across deployments and that, if
embedded in the runtime, would force a particular operational model on
adopters that may not suit their environment. They belong in the layer that
already encodes the deployment's choices about networking, identity, and
orchestration.

## 11. Closing

The runtime is a process with a data directory and a network interface. The
deployment is everything else. Drawing that line cleanly is the precondition
for operating the system with confidence. The runtime is built on the
assumption that the deployment will draw it; this paper exists so that
assumption is shared.
