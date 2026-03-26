# Redaction

## 1. Overview

Detection identifies what is sensitive; redaction determines what to do about it. The distinction between a basic redaction tool and a production-grade platform lies in the ability to apply redaction with contextual awareness: understanding not just that a name appears in a document, but whose name it is, why it matters, and whether the surrounding regulatory and organizational policy requires its removal.

## 2. Context-Aware Redaction

### 2.1 Instance-Level Precision

The platform must distinguish between occurrences of the same entity across different contexts. Redacting "John Smith" in one document should not require redacting every occurrence of the name across an entire corpus. Redaction decisions must be scoped to the relevant instance, document, or case.

### 2.2 Conditional Redaction

Redaction rules must support conditional logic:

- **Document-type conditions**: Apply medical redaction policies only when the document type is classified as a health record. The pipeline envelope carries the original content metadata (MIME type, filename, custom key-value pairs) through every stage, making these signals available to redaction rules without re-reading the original content.
- **Temporal conditions**: Redact specific time segments in audio content.
- **Metadata conditions**: Activate or suppress rules based on custom metadata attached at upload (e.g., department, classification level, jurisdiction).

### 2.3 Relationship-Aware Redaction

Advanced redaction scenarios require reasoning over relationships between entities: redacting all names associated with a specific case identifier, or redacting all communications involving a particular individual across a document set.

### 2.4 Confidence-Driven Redaction

Each detection carries a confidence score from the underlying model or pattern matcher. Redaction rules should be able to set confidence thresholds: entities above the threshold are redacted automatically, while entities below it are flagged for review or skipped. This enables a tunable precision/recall tradeoff per entity type.

## 3. Regulatory Policy Templates

Predefined policy templates aligned to common regulatory frameworks provide a starting point for organizations deploying the platform. Each template is a JSON definition of entity types, pattern identifiers, and redaction actions that collectively satisfy the requirements of a specific regulation.

Templates included in the repository:

- **HIPAA**: Protected health information in medical records, communications, and claims.
- **GDPR**: Personal data of EU residents across all modalities.
- **PCI-DSS**: Payment card data in documents, images, and structured records.
- **CCPA**: Personal information of California residents.

Templates are not enforced by default. Organizations select and extend templates through the policy configuration, adding organization-specific rules or narrowing thresholds as needed.

## 4. Review Integration

The runtime does not provide a review interface, but it accepts review decisions as an optional input to the pipeline. A review decision is a list of actions (accept, reject, or modify) applied to individual redaction entries before export.

When review input is provided:

- **Accepted** redactions proceed to export unchanged.
- **Rejected** redactions are removed: the original content is restored for those regions.
- **Modified** redactions replace the automated output with a reviewer-specified alternative (different mask text, adjusted bounding box, narrower time range).

When no review input is provided, all automated redaction decisions are applied as-is. This keeps the engine stateless with respect to review workflow: the external review service is responsible for presenting decisions to reviewers, collecting responses, and passing the result back to the runtime for a final export pass.

Each review action should carry the reviewer identity and a timestamp so that the audit trail records who approved or modified each decision.

## 5. Redaction Versioning and Rollback

### 5.1 Versioned Redaction State

The platform must maintain versioned snapshots of redaction state for each piece of content. Each modification, whether automated or manual, produces a new version. Prior versions must remain accessible for comparison, audit, and rollback.

### 5.2 Rollback

Before export, any redaction decision must be reversible. The engine must support rolling back individual redactions or restoring an entire document to a previous redaction state. After export, rollback is no longer available: the exported artifact is final, and any corrections require re-processing from the original content.
