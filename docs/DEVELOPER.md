# Developer Experience

## 1. Overview

Platform adoption scales with developer experience. Organizations evaluate redaction platforms not only on detection accuracy but on how quickly they can integrate the platform into existing systems, automate workflows, and extend capabilities to meet domain-specific requirements.

## 2. Core Interfaces

### 2.1 REST API

The runtime exposes a REST API through the server layer for content submission, pipeline execution, run management, and result retrieval. The API is the primary integration surface.

API versioning must follow a clear strategy (URI-based or header-based) with documented deprecation timelines. Breaking changes must not be introduced without a major version increment and a migration period.

### 2.2 SDKs

Official client libraries for Python and JavaScript (at minimum) should wrap the REST API with idiomatic interfaces, type safety, and built-in error handling. SDKs are maintained as separate packages outside the runtime repository.

### 2.3 Authentication and Rate Limiting

Authentication and rate limiting are provided by the surrounding infrastructure (see [INFRASTRUCTURE.md](INFRASTRUCTURE.md) Section 4.2). The runtime accepts an authenticated actor identity with each request. Rate limiting should be enforced per client and per tenant by the API gateway or reverse proxy layer.

### 2.4 Webhooks and Events

An event-driven notification system should allow consumers to subscribe to processing lifecycle events (content ingested, detection complete, redaction applied) without polling. Webhook delivery is the responsibility of the surrounding infrastructure; the runtime emits lifecycle events that an external service can relay.

## 3. Tooling

### 3.1 CLI

The `nvisy-cli` crate provides a command-line interface for common operations: starting the server, submitting content, querying run status, and downloading results. The CLI wraps the REST API and is suitable for scripting, automation, and developer workflows.

### 3.2 Sample Policies and Synthetic Data

A library of sample redaction policies (see [REDACTION.md](REDACTION.md) Section 3) and a synthetic data generator should be available to accelerate development and testing. Developers should be able to exercise the full pipeline against realistic but non-sensitive data without access to production content.

## 4. Configuration

### 4.1 Runtime Configuration

The engine accepts a TOML-based runtime configuration that controls subsystem behavior: LLM provider selection, OCR settings, speech-to-text parameters, and engine-level tuning (channel buffer sizes, parallelism limits). Configuration is structured into discrete sections for each subsystem, with clear separation between engine-level settings and provider-specific settings.

### 4.2 Configuration Validation

Before a pipeline executes, the runtime configuration is validated to catch structural errors early: empty API keys, invalid buffer sizes, and other constraint violations. Validation errors are surfaced as structured error messages identifying the specific field and constraint that was violated.

### 4.3 Environment Variable Resolution

API keys and other secrets can be injected from environment variables rather than stored in configuration files. When a configuration field is empty, the engine checks a corresponding environment variable before rejecting the configuration. This supports deployment patterns where secrets are managed through infrastructure (Kubernetes secrets, CI/CD variables) rather than config files.

### 4.4 Per-Request Overrides

Each pipeline execution can include configuration overrides that are merged with the base runtime configuration. Overrides replace entire sections: they do not partially merge individual fields within a section. This allows callers to adjust provider selection, model parameters, or feature flags on a per-request basis without modifying the persistent configuration.

## 5. Advanced Capabilities

### 5.1 Risk Scoring

Documents and datasets should be scored by aggregate privacy exposure level, enabling organizations to prioritize review effort and allocate resources toward the highest-risk content.

### 5.2 Smart Redaction Suggestion

Rather than applying maximal redaction, the engine should be capable of suggesting the minimal set of redactions required to satisfy a given regulatory standard. This preserves data utility while meeting compliance obligations.

### 5.3 Semantic Redaction

Beyond named entity redaction, the engine should support redaction of semantic categories: references to rare diseases, specific legal proceedings, or proprietary methodologies, that carry sensitivity not through the presence of a specific identifier but through their meaning in context.

### 5.4 Synthetic Data Replacement

Rather than replacing sensitive content with black bars or placeholder tokens, the engine should support replacement with realistic synthetic alternatives: generated names, addresses, dates, and other values that preserve the statistical and structural properties of the original data while eliminating re-identification risk.
