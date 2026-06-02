# nvisy-document

Whole-document carrier and pipeline orchestrator for Nvisy.

Owns the [`Document<M>`] tree, its [`Audit`] / provenance, the
[`Policy`] store, the ingestion path (importer, exporter, registry),
and the pipeline runner that drives a document through extraction,
detection, deduplication, redaction, and validation phases.

Depends on `nvisy-core` for the atomic types (primitives, Entity,
Modality) and on `nvisy-toolkit` for the composable components the
runner plugs in (recognizers, dedup layers, checks, redaction
strategies).
