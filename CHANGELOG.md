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

- **nvisy-cli:** CLI entry point for the nvisy API server
- **nvisy-codec:** File-format codecs — read, edit, and write documents
- **nvisy-core:** Domain types, traits, and errors
- **nvisy-engine:** DAG compiler and executor for pipeline graphs
- **nvisy-identify:** Entity ontology types and detection layers
- **nvisy-ontology:** Domain data types, entity taxonomy, and spatial primitives
- **nvisy-pattern:** Built-in regex patterns and dictionaries for PII/PHI detection
- **nvisy-ocr:** OCR backend trait and provider integration (oar-ocr local, Python bridge)
- **nvisy-python:** PyO3 bridge for AI NER/OCR detection via embedded Python
- **nvisy-rig:** LLM/VLM-driven detection, redaction, and OCR backends
- **nvisy-server:** HTTP server exposing the Engine pipeline via REST endpoints

[Unreleased]: https://github.com/nvisycom/runtime/commits/main
