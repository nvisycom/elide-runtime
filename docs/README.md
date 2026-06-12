# A System for Multimodal PII Detection and Redaction

## Abstract

This document series describes the conceptual architecture of a system
for detecting and redacting personally identifiable information across
heterogeneous data: free text, tabular records, still images, and
recorded audio. The system addresses three problems that a
single-modality redactor cannot. First, sensitive content surfaces
through different carriers in each modality (a span of characters, a
cell value, a region of pixels, an interval of waveform), and any
unified treatment must respect those carriers rather than reduce them
to a common substrate. Second, no single detection technique is
sufficient: deterministic patterns recover structured identifiers with
high precision, statistical models recover unstructured entities with
useful recall, and generative models recover context-dependent
mentions that neither of the prior two can frame; the system therefore
admits a pluralistic detection layer in which multiple recognisers
contribute to a single annotation set. Third, the regulated settings
this system targets demand that redaction be auditable and, in
specific cases, reversible by an authorised party, so the system
treats the rewrite operation itself as a first-class object with a
declared kind (suppression, anonymization, pseudonymization) and a
recorded provenance. The overall pipeline separates detection from
redaction into two distinct phases: detection produces an immutable
record of what was found and why; redaction consumes that record,
admits human overrides, and writes format-preserving output together
with the audit trail that justifies every change.

## Reader's guide

The remaining documents in this series each take one slice of the
system and develop it in isolation. They are independent and may be
read in any order, though the order below reflects the flow of a
document through the system.

| Document | Subject |
| --- | --- |
| Pipeline | The two-phase decomposition of the system into a detection phase that produces an immutable artifact and a redaction phase that consumes it, together with the contract between them and the role of human review at the seam. |
| Ingestion | The process by which a raw uploaded file becomes a typed, addressable handle on which the rest of the pipeline operates, including format identification, structural extraction across modalities, and the preservation of descriptive metadata. |
| Detection | The composition of rule-based, statistical, and generative recognisers into a single layer that produces a unified set of entity annotations, including the treatment of overlap, disagreement, and confidence between recognisers. |
| Redaction | The translation of detected entities into concrete rewrites or removals on the original document, the catalogue of operator kinds, the role of policy in selecting an operator per entity, and the conditions under which a redaction is reversible. |
| Compliance | The semantics of the audit trail, the distinction between what the runtime itself guarantees and what the surrounding deployment must provide, retention policy, and the explainability requirements that govern every recorded decision. |
| Infrastructure | The deployment shape of the system, the external services it depends on for storage, key management and access control, and the boundaries between concerns the runtime owns and concerns the surrounding environment owns. |
| Developer | The extension surfaces through which a practitioner adds support for a new input format, a new recogniser, or a new redaction operator, and the configuration model that governs per-deployment behaviour. |

## Glossary

The terms below are used throughout the series with the meanings given
here. They are intended as conceptual definitions, not as references
to any particular interface.

- **Modality** — a class of data carrier with its own internal
  structure and its own notion of location: text, tabular records,
  still images, and recorded audio are the four modalities treated by
  this system.
- **Entity** — a single occurrence of sensitive information within a
  document, located in one modality, of one declared kind (person
  name, identifier, face region, spoken interval, and so on).
- **Location** — the modality-specific coordinate that identifies
  where an entity lives in its host document: a character span, a row
  and column, a pixel region, or a time interval.
- **Redaction** — the act of replacing, removing, or otherwise
  altering an entity in the host document so that it no longer
  conveys the sensitive information it originally carried.
- **Anonymization** — a redaction that severs the link between the
  redacted document and the original entity in a way that the system
  itself cannot reverse.
- **Pseudonymization** — a redaction that substitutes the entity for
  a token or surrogate value, where the substitution can be reversed
  by a party in possession of the appropriate key or mapping.
- **Deanonymization** — the inverse operation to a pseudonymizing
  redaction, available only where the original redaction was declared
  reversible and the reverser holds the requisite authority.
- **Audit trail** — the record kept by the system of every entity
  detected, every operator selected, every override applied, and
  every byte changed, structured so that any single redaction can be
  explained after the fact.
- **Override** — a human decision, supplied at the seam between
  detection and redaction, that accepts, rejects, replaces, or adds
  to the automated finding before it is committed to output.
- **Policy** — a declarative specification that maps an entity (by
  kind, by confidence, by document context) to the operator that
  should redact it, allowing the system's behaviour to be tuned
  without modification to its detection or redaction layers.
