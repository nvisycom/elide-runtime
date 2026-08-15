# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Multimodal redaction pipeline for sensitive data detection across
  text, images, audio, and structured documents (PDF, DOCX, XLSX, CSV,
  JSON, plain text).
- Layered detection: regex + dictionary patterns, NER, OCR, and LLM
  classification.
- Context-aware redaction operators: mask, replace, hash, encrypt,
  blur, block, pixelate, policy-driven with per-entity confidence
  thresholds.
- Deployment-owned NER, LLM, OCR, and STT recognizer/enricher lineups
  (deployment picks the providers; the wire only toggles them on or off).
- HTTP client integration with the inference services shipped separately
  from [nvisycom/elide-bento](https://github.com/nvisycom/elide-bento)
  via the `elide-bento` git dependency.
- `all-modalities` feature on `elide-pipeline` enabling every shipped
  modality (tabular, image, audio, container documents) under one
  toggle; `default` delegates to it. Text is unconditional and needs
  no toggle. `codec-mp3` and `codec-pdf-render` stay opt-in: MP3
  patent licensing may not be satisfiable downstream, and PDF
  rasterisation pulls in a native rendering dependency.

### Changed

- The `elide-bento` dependency points at
  [nvisycom/elide-bento](https://github.com/nvisycom/elide-bento),
  matching the repository rename. The former `nvisycom/bento` URL
  still redirects, so this names the canonical target rather than
  fixing a break.

### Rust crates (`crates/`)

- **elide-governance:** wire schema for redaction governance (rules,
  predicates, operators).
- **elide-wire:** wire schemas for plan (analyzer parameters) and file
  (document envelope); peer to `elide-governance`, which consumers
  depend on directly. Consumed by SDKs on both sides of the HTTP
  boundary.
- **elide-template:** ready-to-run policy templates for common
  regulatory postures (HIPAA §164.514, GDPR Article 9, PCI DSS,
  CCPA / CPRA).
- **elide-pipeline:** stateless pipeline: decode, analyze, apply. Wraps
  elide and hosts the per-modality orchestrator plus the
  deployment-side NER / LLM recognizer configuration. The umbrella
  entry point: re-exports elide and `elide-governance` so callers
  reach everything from here.
[Unreleased]: https://github.com/nvisycom/elide-runtime/commits/main
