# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Multimodal redaction pipeline for sensitive data detection across
  text, images, audio, and structured documents (PDF, DOCX, XLSX, CSV,
  JSON, plain text)
- Layered detection: regex + dictionary patterns, NER, OCR, and LLM
  classification
- Context-aware redaction operators: mask, replace, hash, encrypt,
  blur, block, pixelate, policy-driven with per-entity confidence
  thresholds
- Deployment-owned NER and LLM recognizer lineups (deployment picks
  the providers; the wire only toggles NER or LLM on or off)

### Crates

- **nvisy-schema:** wire types (policy, context, plan, file). Consumed
  by SDKs on both sides of the HTTP boundary.
- **nvisy-core:** deployment-side runtime configuration (NER and LLM
  recognizer lineups, error vocabulary).
- **nvisy-engine:** stateless pipeline: decode, analyze, apply. Wraps
  elide and hosts the per-modality orchestrator.
- **elide-bento:** BentoML-hosted NER and OCR backends implementing
  elide's recognizer traits.

[Unreleased]: https://github.com/nvisycom/runtime/commits/main
