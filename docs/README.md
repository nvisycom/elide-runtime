# Nvisy Runtime

**A data protection runtime for AI pipelines.**

---

## Abstract

AI-powered products handle sensitive data at every stage — ingestion, transformation, enrichment, and storage. PII in documents, faces in images, credentials in logs, and financial data in spreadsheets all require detection, classification, and redaction before downstream consumption.

**Nvisy Runtime** is a Rust-native data protection platform that treats sensitive data detection as a first-class pipeline primitive. It provides a DAG-based execution engine, typed data primitives with lineage tracking, regex and AI-powered entity detection, configurable redaction policies, and a pluggable connector system — all designed for throughput, correctness, and auditability.

---

## Problem Statement

### 1. Sensitive data is everywhere in AI pipelines

Documents, images, API responses, and model outputs all carry PII, PHI, financial data, and credentials. Manual redaction doesn't scale. Teams need automated, configurable detection and redaction that runs inline with their data pipelines.

### 2. Detection requires multiple methods

Regex patterns catch structured data (SSNs, emails, credit cards). AI-powered NER catches unstructured entities (names, addresses, medical terms). Checksum validation reduces false positives. A production system needs all three, composable in a single pipeline.

### 3. Redaction must be auditable

Compliance (GDPR, HIPAA, PCI-DSS) requires proof of what was detected, what was redacted, and how. Every detection and redaction action must produce an audit trail with full lineage.

### 4. Performance matters

Data protection runs on every record. The runtime must handle high throughput without becoming a bottleneck. Rust provides the performance foundation; Python extensions handle AI workloads where model quality matters more than latency.

---

## Design Principles

### Typed data primitives

Every data object flowing through a graph is typed: `Document`, `Blob`, `Entity`, `Redaction`, `Policy`, `Audit`, `Image`. Primitives carry metadata and enforce structural contracts at compile time (Rust) and runtime (serde validation).

### DAG-based execution

Graphs are directed acyclic graphs of nodes (sources, actions, targets). The engine resolves dependencies, manages concurrency, handles retries, and tracks execution state.

### Regex + AI detection

Built-in regex patterns detect structured sensitive data. Python-based NER (via PyO3) detects unstructured entities. Both produce the same `Entity` type, composable in a single pipeline.

### Plugin architecture

Connectors, actions, and loaders register through a plugin system. Each plugin bundles its capabilities under a namespace. The engine resolves references at compilation time.

### Audit-first

Every detection and redaction produces an `Audit` record. Policies define what to detect and how to redact. The audit trail provides full lineage from source document to redacted output.

---

## Core Concepts

### Entities

An **Entity** is a detected piece of sensitive data: its category (PII, PHI, financial, credentials), type (SSN, email, face), value, confidence score, detection method, and location within the source document or image.

### Policies

A **Policy** defines detection and redaction rules: which entity categories to scan, minimum confidence thresholds, and per-type redaction methods (mask, replace, hash, encrypt, remove, blur, block, synthesize).

### Graphs

A **Graph** is a DAG of nodes. Source nodes read data, action nodes detect/redact/classify, and target nodes write results. Graphs are defined as JSON and compiled into execution plans.

### Connectors

Connectors implement the source and target interfaces. The object storage connector (S3) handles file ingestion and output. Additional connectors register through the plugin system.

---

## Deployment

The server (`nvisy-server`) is a short-lived Axum HTTP server. It accepts graph definitions, executes them, and reports status. Designed for containerized deployment — the main server spins it up, feeds work, waits for completion.

---

## Project Status

Active development. The Rust runtime, detection engine, and server are implemented. AI-powered detection runs via Python extensions.

---

## License

Apache License 2.0. See [LICENSE.txt](../LICENSE.txt).
