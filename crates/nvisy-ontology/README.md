# nvisy-ontology

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Domain data types for the Nvisy platform — per-modality coordinate
types, entity / annotation taxonomy, redaction policies, and the
audit provenance shared across every crate in the workspace.

## Overview

`modality::*` defines the per-modality coordinate types (`Text`,
`Image`, `Audio`, `Tabular`) and the `Modality` marker trait every
generic container parameterises over. Per-modality block payloads
(`TextBlock`, `ImageBlock`, …) implement the shared `ModalityBlock`
contract.

`document::*` carries the in-memory pipeline state for one processed
document: `Document<M>` holds `meta`, an ordered `Vec<Block<M>>`,
user `Annotation<M>`s, labels, and the embedded `Audit<M>`.
`Block<M>` wraps a per-modality `kind` payload alongside
source-mapping `Span<M>`s and an optional confidence.

`entity::*` defines the entity taxonomy (`EntityCategory`,
`EntityKind`, `EntitySensitivity`), the detection-result type
`Entity<M>`, user-supplied `Annotation<M>` + `AnnotationKind` +
`AnnotationStrength`, the per-recognition / per-refinement method
enums, and `ContentSource`.

`provenance::*` records what the pipeline did. `Audit<M>` ships
the per-document compliance trail as `records: Vec<EntityRecord<M>>`
— each record bundles a detected `Entity<M>` with the optional
`AuditEntry<M>` produced for it during redaction. `AnyAudit` is
the modality-erased enum used by persistence.

`policy::*` describes how a detected entity should be redacted.
Per-modality `*Strategy` enums (`TextStrategy::Mask`,
`ImageStrategy::Blur`, …), `EntitySelector` for matching entities
to rules, `Action<M>` for deciding what to do (redact / suppress /
…), `Condition` predicates, and the top-level `Policy<M>` container
plus `RuleRank` for tie-breaking.

`primitive::*` carries cross-cutting value types — `Confidence`,
`ConfidenceThreshold`, `LanguageTag`, `LanguageDetection`,
`LanguageProvenance`, `LanguageSpan`, `BoundingBox`, `Polygon`,
`Dimensions`, `NormalizedBoundingBox`, `TimeSpan`, `Color`, `Dpi`.

`context::*` defines runtime reference-data: a `Context` of
`ContextEntry`s, each carrying a `ContextEntryData` tagged by
domain — `Biometric`, `Geospatial`, `Analytic`, `Reference`,
`Temporal`, `Document`.

## Documentation

See [`docs/`](../../docs/) for architecture, security, and API documentation.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
