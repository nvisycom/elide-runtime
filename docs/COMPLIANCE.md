# Compliance and Audit

## 1. Framing

Compliance is a sociotechnical concern, not a software feature. No runtime can deliver
"compliance with GDPR" or "SOC 2 certification" on its own; only an organization can,
through audit-defensible processes that span legal review, organizational policy,
operational controls, personnel training, and a body of evidence far larger than any
single piece of software. A redaction engine cannot certify a hospital, a bank, or a
data processor; it can only supply one of the load-bearing components such a
certification depends on.

This document describes that component. The runtime described here provides an
auditable detection-and-redaction pipeline: a mechanism for identifying sensitive
entities in unstructured content, deciding what to do with each one under a stated
policy, executing those decisions, and recording every step in a form that a downstream
auditor can read, replay, and challenge. The pipeline is a building block. The
compliance posture is the building.

The remainder of this paper describes the conceptual model that makes the runtime
auditable, the boundaries of what it is responsible for, and the boundaries of what it
deliberately leaves to layers above and below it. The intended reader is a compliance
engineer, internal auditor, or external assessor evaluating whether this component is
suitable for inclusion in a controlled environment.

## 2. The Audit Trail

Every entity that survives detection generates an audit record. By "survives detection"
we mean that one or more recognizers asserted the entity's existence, the assertion was
not filtered out by confidence thresholds or deduplication, and the entity entered the
decision stage. Whether the entity was then redacted, suppressed, or passed through
unchanged, an audit record is produced.

Each record captures five dimensions:

**Identity.** A stable identifier for the entity instance, scoped to the redaction pass.
Re-running the same input under the same configuration produces the same identifiers
for the same entities. The identifier is the handle by which downstream systems refer
to the entity for review, override, or recovery.

**Provenance.** Which recognizers contributed to the detection, and what evidence each
one provided. If a phone number was detected by both a pattern recognizer and a
named-entity recognizer, both contributions are recorded. The evidence captures
sufficient detail to explain *why* the recognizer fired: the matched span, the
confidence it assigned, and the rule or model identifier it was operating under.

**Decision.** Which policy rule matched and what action it dictated. If an override
modified the decision, the override is recorded alongside the original rule outcome,
together with the authority under which the override was applied. A decision is never
recorded without its justification.

**Execution.** Whether the decision was carried out, suppressed, or failed. What the
resulting replacement was, if any. Whether the operation is reversible and, if so,
which reversibility mechanism applies. An execution failure is itself an audit record,
not an absence of one.

**Timestamp.** When the decision was made and when execution occurred. These are
distinguished because the runtime supports decision review independent of execution,
and the two times can diverge under human-in-the-loop workflows.

The audit is append-oriented within a redaction pass: records are produced in order and
not rewritten. It does not provide cryptographic tamper-evidence at the runtime layer.
Tamper-evidence is a property of storage, not of the producer, and is addressed in the
deployment scope below.

## 3. Policy as Code

Compliance rules in this system are programmatic, not declarative tags. A policy is an
ordered sequence of rules. Each rule has a set of conditions evaluated against entity
attributes, document labels, and metadata, and an action prescribed when the conditions
are met. The first matching rule wins; subsequent rules are not considered for that
entity. Entities for which no rule matches receive a default action specified by the
policy itself.

This design is deliberately audit-friendly. Every decision the system makes can be
reduced to a single sentence: "rule N fired against entity E because of conditions C,
prescribing action A." The rule is the citation. The conditions are the evidence. The
action is the disposition. An auditor reconstructing a decision does not need to
understand the engine's evaluation strategy, only the rule that won.

The order of rules is significant and visible. A policy author who places a broad
allow-listing rule before a narrow redaction rule has expressed an intentional ordering
that an auditor can read directly from the policy text. There is no hidden priority
system, no implicit weighting, no opaque conflict resolution.

Policies are versioned. Each redaction pass records the policy version under which it
ran. A decision made under an old policy can be reproduced exactly by replaying it
against the same detection artifact under the same policy version, even after the
policy has evolved.

## 4. The Runtime, Deployment, and Product Scope Split

Compliance work spans many layers. A clear statement of which layer is responsible for
which control is the most useful artifact a runtime component can provide to its
operators. The following split is the position this runtime takes:

| Concern                                | Runtime | Deployment | Product |
|----------------------------------------|---------|------------|---------|
| Per-entity audit records               | Yes     |            |         |
| Persistent retention of artifacts      | Yes     |            |         |
| Programmatic query of past decisions   | Yes     |            |         |
| Reversibility primitives               | Yes     |            |         |
| Authentication and authorization       |         | Yes        |         |
| Encryption at rest                     |         | Yes        |         |
| Network isolation                      |         | Yes        |         |
| Log forwarding to SIEM                 |         | Yes        |         |
| Key management and rotation            |         | Yes        |         |
| Multi-tenant separation                |         | Yes        |         |
| Append-only audit storage              |         | Yes        |         |
| Reviewer workflows and UI              |         |            | Yes     |
| Regulatory policy templates            |         |            | Yes     |
| Legal-hold integration                 |         |            | Yes     |
| Certification documentation            |         |            | Yes     |
| Subject-access request workflows       |         |            | Yes     |

The runtime layer provides the primitives. The deployment layer provides the
operational security envelope. The product layer provides the regulatory framing and
human workflows. A specific organization's compliance posture is the composition of all
three. No single layer is sufficient, and no single layer can substitute for another.
A runtime that claims to provide tamper-evident logging without an append-only storage
layer underneath it is misrepresenting itself, and the same misrepresentation applies
to any product that claims regulatory compliance without operational controls behind
it.

## 5. Reversibility and the Right to Deletion

Redaction operators differ in whether they preserve the possibility of recovery. Some
operators are reversible by design: encryption with a deployment-held key produces a
ciphertext from which the original can be recovered, given the key. Others are
deliberately irreversible: one-way hashing, span removal, and generic placeholder
substitution discard the original and cannot reconstruct it from the redacted output.

This distinction is consequential under regulatory regimes that grant data subjects a
right to deletion. If a subject requests deletion of their data and that data was
redacted using a reversible operator, the cipher key material is itself a copy of the
subject's data, and the request cannot be honored merely by deleting the redacted
output. If the same data was redacted using an irreversible operator, the redacted
output contains no recoverable copy and is, for the purpose of the deletion request,
already deleted.

The runtime exposes this distinction in the audit record. Every execution entry states
whether the operator used was reversible, and if so, the identity of the key or recovery
artifact under which reversal is possible. A deployment can therefore answer, for a
given subject, the question "which records pertaining to this subject are reversibly
redacted, and where do the keys live?" The answer to that question is the
deletion-completion checklist.

The runtime does not itself enforce deletion. Key destruction, cascading deletes, and
revocation are deployment-side controls. The runtime provides the inventory.

## 6. Explainability

For each redaction, the system can answer five questions:

1. *Which entity was detected.* The identifier, type, and location of the entity in the
   input.
2. *Why it was detected.* The recognizers that contributed and the evidence each
   provided.
3. *Why it was redacted.* The policy rule that matched and the conditions under which
   it fired.
4. *What was done.* The operator that was applied and the replacement that was emitted.
5. *How to reverse it, where applicable.* The reversibility mechanism and the location
   of the recovery material.

These five answers are the minimum standard for audit-defensible automation in
regulated contexts. A system that can answer some but not all of them places the
unanswered questions on the human operator, who must then maintain external records to
fill the gap. A system that answers all five places the explanation entirely within the
artifact the auditor is already reviewing.

## 7. Retention Semantics

The runtime persists detection and redaction artifacts indefinitely by default. There
is no built-in expiration, no time-to-live, no automatic purge. This is intentional:
retention policy is a regulatory and contractual question that varies by jurisdiction,
industry, data category, and data subject. A runtime that imposes retention defaults
imposes opinions that may conflict with the operator's obligations.

What the runtime does provide is a clear separation between retention scopes. Raw
content has a separate retention scope from redacted content, which has a separate
retention scope from audit records. A deployment that wishes to delete raw content
after seven days while retaining audit records for seven years can do so without
sacrificing the audit trail's integrity, because the audit record refers to the raw
content by identifier rather than by inclusion. Deleting the raw content does not
invalidate the audit; it simply means the original input is no longer available for
re-examination, while the record of what was decided about it remains.

Retention enforcement, including legal-hold overrides that prevent deletion of
otherwise expired records, is a deployment-side concern. The runtime exposes the
boundaries; the deployment enforces the policy.

## 8. What Is Intentionally Not in Scope

The runtime deliberately does not include the following:

**Regulatory templates.** No preconfigured policy for HIPAA, GDPR, PCI, or any other
regime is shipped with the runtime. Users author their own policies. The reason is
that a regulatory template is a legal interpretation, and legal interpretations
require legal authorship. A runtime that ships a "HIPAA policy" invites operators to
treat it as a legal opinion, which it cannot be.

**Cryptographic audit-log tamper-evidence.** Hash-chained, signed, or
write-once-read-many storage of audit records is achievable, but it is a property of
the storage layer, not the producer. The runtime emits records; the deployment chooses
where to put them. A deployment that requires tamper-evidence pairs the runtime with
append-only storage and a hash-chain or signing layer.

**Reviewer workflows.** Human-in-the-loop review, queue management, reviewer
assignment, escalation, and adjudication are user-interface concerns belonging to a
downstream product. The runtime exposes the decision points; the product builds the
workflow around them.

**Forward-secret retention.** Key rotation, cryptographic erasure, and forward
secrecy for reversible operators depend on key management practices that vary by
deployment. The runtime does not assume a key lifecycle.

The runtime takes the position that bolting these capabilities in would make the
boundary between general-purpose primitives and product-specific assumptions
disappear, leaving a component that is neither a clean primitive nor a complete
product. The boundary as drawn is intentional.

## 9. Architectural Commitments

To support the guarantees above, the runtime makes four architectural commitments that
an auditor can rely on without inspecting the implementation:

**Decisions are deterministic.** Given the same detection artifact, the same policy,
and the same set of overrides, the same decisions are produced. Replay is exact.
Non-determinism, where it exists, is confined to the detection stage and is exposed in
the audit record so that a re-detection producing different evidence is visible as
such.

**Audit records reference the immutable detection artifact.** The question "what was
the input to this decision" always has a definite answer, because the decision is
expressed against an artifact that is not rewritten after the fact. Detection happens,
the artifact is sealed, and decisions reference the sealed artifact.

**Reversibility is a first-class operator property.** Every operator declares whether
it is reversible. A policy author selecting an operator selects a reversibility
posture as part of that choice, and the auditor sees the posture in the record.
Reversibility is not an afterthought attached to operators that happen to support it.

**Suppressed redactions are still recorded.** A rule that matches an entity but
prescribes no rewrite still produces an audit record. The absence of a rewrite is
itself a decision, and the runtime records it as such. An auditor reading the trail
sees not only the entities that were redacted but also the entities that were
deliberately allowed through, with the rule that permitted them.

These commitments together describe what the runtime guarantees to any layer above it.
They are the surface against which an audit is conducted.
