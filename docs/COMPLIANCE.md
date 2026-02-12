# Compliance & Audit

## 1. Overview

Enterprises do not purchase redaction tools; they purchase compliance guarantees. The value of automated redaction is realized only when the organization can demonstrate — to regulators, auditors, and legal counsel — that sensitive data was identified, handled, and redacted in accordance with applicable policy.

This requires two complementary capabilities: a policy engine that encodes regulatory and organizational rules into executable redaction policies, and an audit system that records every decision, action, and outcome with sufficient detail to reconstruct the chain of custody for any piece of content.

## 2. Policy Engine

### 2.1 Policy Definition

The platform must provide a policy builder that enables administrators to define redaction rules without writing code. Policies should express conditions over entity types, document classifications, confidence thresholds, and organizational context.

### 2.2 Regulation Packs

Prebuilt policy packs aligned to common regulatory frameworks should be available out of the box:

- **HIPAA**: Protected health information in medical records, communications, and claims.
- **GDPR**: Personal data of EU residents across all modalities.
- **PCI-DSS**: Payment card data in documents, images, and structured records.
- **CJIS**: Criminal justice information in law enforcement contexts.
- **CCPA**: Personal information of California residents, including the right to deletion and opt-out of sale.
- **FERPA**: Student educational records and related identifiers.

### 2.3 Policy Simulation

Before a policy is applied to production data, administrators must be able to simulate its effect — previewing what would be redacted across a representative sample. This "dry run" capability reduces the risk of over-redaction or under-redaction in production.

### 2.4 Policy Versioning and Approval

Policies must be versioned, with a full history of changes. Modifications to active policies should require approval through a configurable workflow before taking effect.

## 3. Explainability

Every redaction decision must be explainable. The system must record and surface:

- **What was redacted**: The specific content span, region, or audio segment.
- **Why it was redacted**: The triggering rule, pattern, or model prediction.
- **Which model version**: The exact version of any ML model involved in the decision.
- **Confidence level**: The detection confidence associated with the decision.
- **Who reviewed it**: The identity of any human reviewer who approved, rejected, or modified the decision.
- **When it was processed**: Timestamps for each stage of the pipeline.

## 4. Audit Trails

### 4.1 Immutability

Audit logs must be append-only and tamper-evident. Once a record is written, it cannot be modified or deleted.

### 4.2 Chain of Custody

The audit system must maintain a complete chain of custody for every piece of content: from ingestion, through detection and redaction, to export. Every access event — who viewed the content and when — must be recorded.

### 4.3 Reporting

The platform must generate compliance reports suitable for submission to regulators and internal audit teams. Reports should include:

- Redaction statistics by entity type, document category, and time period
- Policy adherence metrics
- Reviewer activity and approval rates
- Exceptions and overrides

### 4.4 SOC 2 Readiness

Logging infrastructure must meet the requirements of SOC 2 Type II certification, including continuous monitoring, access controls, and retention policies.

## 5. Data Retention Policies

### 5.1 Original Content

The platform must enforce configurable retention policies for original (pre-redaction) content. Organizations must be able to specify maximum retention periods after which originals are permanently deleted. Zero-retention mode — in which originals are discarded immediately after processing — must be available for environments where persistent storage of sensitive content is prohibited.

### 5.2 Redacted Output

Redacted artifacts may be retained independently of originals, subject to their own retention schedule. The platform must track the lifecycle of each artifact and enforce automated deletion at expiry.

### 5.3 Audit Logs

Audit log retention must be configurable separately from content retention. Regulatory frameworks often require audit records to be retained for longer periods than the underlying data (e.g., seven years for HIPAA, six years for SOX). Audit logs must never be deleted before their configured retention period expires, regardless of content deletion status.
