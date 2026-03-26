# Infrastructure

## 1. Overview

The regulated industries that require multimodal redaction: healthcare, legal, government, and financial services, impose stringent requirements on where and how data is processed. This document describes the surrounding infrastructure and services that should accompany the runtime in production deployments.

## 2. Deployment Models

The runtime is deployment-agnostic. The surrounding infrastructure determines which model applies:

- **Cloud-hosted**: Managed deployment in the vendor's infrastructure.
- **VPC deployment**: Installation within the customer's own virtual private cloud, ensuring data never leaves their network boundary.
- **On-premises**: Full deployment on customer-owned hardware for organizations with strict data sovereignty requirements.
- **Air-gapped**: Operation without network connectivity, required by certain government and defense use cases. The runtime supports local-only provider configurations for this model.
- **Edge processing**: Lightweight deployment at the point of data capture.

### 2.1 Architecture

The runtime is API-first, supporting both batch and streaming processing modes. All capabilities are accessible programmatically through the server layer, enabling integration into existing enterprise workflows without dependence on a dedicated user interface.

## 3. Performance and Scale

### 3.1 Workload Requirements

The runtime must handle workloads that span orders of magnitude in volume and latency sensitivity:

- Large document sets (thousands to millions of PDFs)
- Long-form audio files
- Real-time stream redaction with sub-second latency targets
- Concurrent processing across multiple tenants or projects

### 3.2 Scaling

The surrounding infrastructure handles horizontal scaling: multiple runtime instances can process independent workloads concurrently without coordination beyond shared storage. GPU acceleration should be provisioned for ML inference workloads where throughput or latency requirements exceed CPU capacity.

### 3.3 Cost Optimization

The runtime optimizes processing cost by routing content through the appropriate detection tier. Simple deterministic pattern matches do not incur the computational cost of ML inference. The tiered processing architecture (regex first, ML models only when deterministic methods are insufficient) reduces cost without sacrificing detection coverage.

### 3.4 Content Storage

The runtime stores content in a registry backed by an embedded key-value store. Raw content bytes and descriptive metadata are persisted in separate storage keyspaces, enabling independent access patterns: metadata lookups do not require reading the full content payload, and content retrieval includes metadata reconstruction automatically.

When content is registered, the registry eagerly detects the MIME type from magic-byte signatures and persists it as part of the metadata. This ensures that format detection signals survive the storage round-trip, even for content where magic bytes are the only available detection method.

Content is addressed by a composite key of actor identity and content source identifier, providing natural tenant isolation at the storage layer.

## 4. Security

### 4.1 Data Protection

Given that the runtime processes the most sensitive data an organization holds, security must be foundational rather than additive:

- **Encryption at rest**: Content stored in the registry supports AES-256-GCM encryption via a pluggable key provider interface. The runtime integrates with external key management systems (AWS KMS, Azure Key Vault, HashiCorp Vault) through the key provider abstraction.
- **Encryption in transit**: The surrounding infrastructure must terminate TLS for all external API communication (reverse proxy, load balancer). The runtime does not terminate TLS itself.
- **Zero-retention processing**: The runtime supports a zero-retention mode in which content is discarded from the registry immediately after pipeline execution completes.
- **Ephemeral compute**: The surrounding infrastructure may provision ephemeral environments (containers, serverless) where the processing environment is destroyed after each job. The runtime is designed to operate in this model.

### 4.2 Access Control

The surrounding infrastructure must provide access control services:

- **Role-based access control (RBAC)**: Fine-grained permissions governing who can configure policies, submit content, and access audit logs.
- **Single sign-on (SSO) and SCIM**: Integration with enterprise identity providers for authentication and automated user provisioning.
- **Data residency controls**: Configuration to ensure that content is processed and stored only within specified geographic regions.

The runtime accepts an authenticated actor identity with each request and enforces tenant isolation at the storage and pipeline level based on that identity. It does not implement authentication or authorization logic itself.

## 5. Multi-Tenancy

### 5.1 Tenant Isolation

The runtime enforces tenant isolation at the data layer: content, metadata, and audit records are keyed by actor identity, ensuring that no actor can access another actor's data. The surrounding infrastructure is responsible for compute isolation (dedicated or partitioned processing resources per tenant) and API-layer tenant scoping.

### 5.2 Tenant-Specific Configuration

Each pipeline execution accepts its own configuration (provider selection, model parameters, policies). The runtime does not maintain persistent per-tenant configuration; this is managed by the calling service and passed per-request.

## 6. Observability

### 6.1 Metrics

The runtime must expose operational metrics covering ingestion throughput, detection latency, redaction processing time, queue depth, error rates, and resource utilization. Metrics must be available in a format compatible with standard monitoring systems (Prometheus, OpenTelemetry, or equivalent).

### 6.2 Distributed Tracing

Each piece of content carries a trace identifier through every stage of the pipeline: ingestion, detection, redaction, and export. Distributed tracing enables operators to diagnose latency bottlenecks, identify failed processing stages, and correlate events across services.

All tracing events use explicit, hierarchical target names following the convention `<crate>::<module>::<submodule>` (e.g. `nvisy_engine::op::import_file`, `nvisy_codec::transform::text`). This enables precise per-module log filtering in production without relying on log levels alone.

### 6.3 Alerting

The surrounding monitoring infrastructure should consume the runtime's metrics and tracing data to trigger alerts on operational anomalies: elevated error rates, processing latency exceeding thresholds, queue backpressure, model inference failures, and storage capacity warnings. Alerts should be deliverable through standard channels (email, webhook, PagerDuty, or equivalent).
