# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Multimodal redaction pipeline for PII/PHI detection across text, images, and documents
- File-format codecs for PDF, DOCX, HTML, Image, XLSX, Audio, CSV, JSON, and plain-text
- Pattern-based entity detection with regex and dictionary matching
- Post-match validators (Luhn checksum, SSN format) to reduce false positives
- Built-in dictionaries for nationalities, religions, currencies, cryptocurrencies, and languages
- DAG compiler and executor for graph-based processing pipelines
- Object store integration with S3 for streaming ingestion and output
- PyO3 bridge for AI-powered NER models (text and image detection)
- Plugin trait system with action/provider registry

### Crates

- **nvisy-core** - Domain types, error types, and plugin trait system
- **nvisy-engine** - DAG compiler, executor, and connection routing
- **nvisy-codec** - File-format handlers with span-based content access
- **nvisy-object** - Cloud storage providers and streaming I/O
- **nvisy-pattern** - Detection patterns, dictionaries, and validators
- **nvisy-pipeline** - Detection, redaction, generation actions, and audit trails
- **nvisy-python** - PyO3 bridge for Python NER models

[Unreleased]: https://github.com/nvisycom/runtime/commits/main
