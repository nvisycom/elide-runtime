# Redaction

A conceptual white paper on the redaction subsystem. The audience is a
privacy engineer evaluating the architecture; the goal is to describe the
model, not the implementation.

## 1. The Redaction Problem

Detection answers *where is the sensitive information*. Redaction answers
the harder question: *what should happen to it*. The two are routinely
conflated, and the conflation hides a design problem. Once a span has been
identified as personal data, the system must transform it, and every
possible transformation costs something.

Naive removal -- deleting the span outright -- destroys downstream utility.
Analytics that counted records, machine-learning workflows trained on
context windows, and human reviewers that relied on document structure all
degrade when arbitrary bytes vanish. Naive replacement -- substituting a
fixed token like `[REDACTED]` -- preserves shape but leaks structure. If
every occurrence of one individual becomes the same token across a
corpus, the redaction itself is a join key: the identifier the operator
was supposed to eliminate.

Different deployments demand different trade-offs. A public data release
wants strong anonymization, biased toward irreversibility. An internal
audit pipeline wants reversible pseudonymization so authorized
investigators can later recover the original value. A financial dataset
wants format-preserving masking so downstream validators continue to pass.
A machine-learning corpus wants synthetic but plausible replacements so
token distributions remain useful. A single redaction operation cannot
serve all four. The runtime therefore treats redaction as a *family of
operators* parameterized by policy. Each operator is a small contract:
given a span and its modality, produce a transformed span and an audit
record describing what happened.

## 2. Operator Taxonomy

Operators divide into two top-level categories based on whether the
original can be recovered from the audit trail.

```
                          Operator
                             |
              +--------------+---------------+
              |                              |
         Anonymizer                     Deanonymizer
              |                              |
   +---+------+------+-----+               (key-held
   |   |      |      |     |                recovery)
 keep mask  hash  replace remove              |
                                          decrypt
```

### 2.1 Anonymizers

Anonymizers transform a span in a way intended to be irreversible from the
redacted artifact alone.

- **Keep** records that an entity was inspected and consciously preserved.
  Not a no-op; absence from the audit trail is semantically different
  from the explicit decision to keep.
- **Replace** substitutes a token (literal string, synthetic value,
  category reference) for the span. The simplest anonymizer; also the
  most prone to leakage when substitution is naive.
- **Mask** transforms the span character by character (or sample by
  sample), preserving format. A masked card number stays the same length
  and still parses as a card number, but its information content is gone.
- **Hash** produces a one-way fingerprint. The original is absent from the
  artifact, but identical inputs produce identical outputs, preserving
  equality joins at the cost of dictionary-attack exposure.
- **Remove** deletes the span entirely; surrounding bytes close around the
  hole. The most lossy and format-disruptive operator; some codecs cannot
  honour it without producing invalid output.

### 2.2 Deanonymizers

Deanonymizers reverse an earlier transformation when the holder presents
sufficient authority, typically a cryptographic key. The canonical case is
**decryption**: a span encrypted by an anonymizer is recoverable later by
the key holder. The ciphertext sits in the artifact in place of the
original; the audit record carries the metadata needed to reverse it.

Reversibility is a first-class operator category, not a feature flag on
individual anonymizers. Reversible operators require key management,
rotation, and access controls; they carry liability anonymizers do not (a
stolen key is a stolen dataset); they are appropriate for internal
pipelines and inappropriate for public releases. Conflating them under a
single "redact" verb hides exactly the property the privacy engineer most
needs to reason about.

## 3. Per-Modality Replacement Semantics

The operator catalogue is uniform, but the concrete meaning of "replace
this span" depends on the modality. Each modality has its own notion of a
span at the byte level and of what replacement looks like.

```
Modality     Span               Replacement primitive
--------     ----               ---------------------
Text         byte range         substitute string (length may change)
Tabular      cell coordinate    rewrite cell, or drop column schema-wide
Image        bounding box       blur | block | mosaic | pixel substitute
Audio        time interval      silence | remove | substitute samples
```

**Text.** A span is a half-open byte range. Replacement substitutes a
string for the matched bytes; surrounding bytes are unchanged. Replacement
length may differ from the original, which is why text redaction batches
require coordinate care (Section 7).

**Tabular.** A span is a cell coordinate or a column identifier for
schema-wide operations. Replacement either rewrites the cell or drops the
column from the output schema.

**Image.** A span is a bounding box. Replacement has several flavors that
are not interchangeable: Gaussian blur preserves rough silhouette,
solid-block redaction preserves nothing, mosaic pixelation preserves
coarse color statistics, and in-place pixel substitution preserves
geometry while destroying identifiable detail. The choice is policy-driven.

**Audio.** A span is a time interval. Replacement can silence the interval
(amplitude zeroed), remove it (the buffer shrinks and downstream
timestamps shift), or substitute alternate samples (pink noise, a tone, a
synthetic utterance).

The operator framework abstracts over these primitives. A policy that says
"mask this entity" produces the correct per-modality output without each
operator carrying modality-specific code. This is why the operator
catalogue is small: every operator must have a meaningful definition in
every modality the system handles. Operators that cannot meet that bar are
pushed into the modality-specific layer.

## 4. Format-Preserving Output

After redaction, the modified artifact re-serializes to its original
container format. A redacted CSV is still a valid CSV; a redacted PDF is
still a valid PDF; a redacted WAV is still a valid WAV. Two properties
follow. First, **non-redacted bytes are preserved**: when the codec
supports it, every byte outside a redaction span is byte-for-byte
identical to the input. Re-encoding the entire artifact would be simpler,
but would subtly alter content the policy never intended to touch --
floating-point columns re-rounded, image compression artifacts shifted,
audio samples re-quantized. The runtime treats avoidable re-encoding as a
bug. Second, **the format contract is preserved**: downstream consumers
that expect a specific column ordering or a PDF whose embedded fonts
match an external reference continue to work.

The cost is that the redaction layer threads its operations through
codec-specific writers that understand how to splice modifications into
the original stream. The benefit is that the runtime can be inserted into
a deployment without forcing every consumer to tolerate a re-serialized
variant of its input.

## 5. Reversibility as a Design Choice

The audit trail records different information depending on whether the
operator was reversible. For an irreversible operator -- masking, hashing,
removal, plain replacement -- the audit stores the *replacement*: what was
written into the artifact, where, and which policy rule selected the
operator. The original value is intentionally absent. If the audit log
contains the original, it becomes a new copy of the personal data, and
the redaction has not reduced exposure, only relocated it.

For a reversible operator, the audit stores enough information to recover
the original when the key is available -- an opaque token or ciphertext
reference, but never the plaintext. This is the difference between a
redacted dataset that can be re-identified by authorized parties and one
that genuinely cannot. The runtime exposes the distinction at the
operator boundary so the choice is deliberate at policy-authoring time,
not an emergent property of how the log was configured.

## 6. Override Flow

The detection artifact is immutable: once detection produces a set of
entities, that set is the canonical record of what was found. Redaction
accepts an override layer that lets a reviewer adjust the decisions
before the destructive write step.

```
   Detection Artifact (immutable)
              |
              v
   +---------------------+
   | Override Resolution |  <-- reviewer / upstream system
   +---------------------+
              |
              v
   Effective Entity Set
              |
              v
   Redaction Batch -> Codec Write -> Audited Artifact
```

The override surface supports four operations:

- **Accept.** Take the detected entity as-is. Default for any entity not
  mentioned by an override.
- **Reject.** Suppress redaction for a specific entity. The detection
  record persists in the audit, marked as suppressed; the artifact bytes
  are left untouched.
- **Replace.** Use a different operator than the one policy selected. A
  policy that defaulted to masking might be overridden to use encryption
  where reversibility is required.
- **Add.** Insert a new entity at a location detection missed. It carries
  an explicit operator and span coordinate; it does not back-fill the
  detection artifact but is recorded as an override-added entity in the
  audit.

Overrides reference entities by their stable identifier from the
detection artifact, not by position or content. This makes overrides
robust to re-runs of detection that might shift coordinates and keeps the
override file meaningful as a standalone record. The override layer is
intentionally narrow: it does not allow editing the content of a detected
entity, changing its type, or rewriting its confidence; those are
modifications to the detection artifact, which is immutable by design.

## 7. The Redaction Batch

A document typically produces many entities. The runtime collects them
into a batch and lets the codec layer decide ordering. For codecs whose
write step *shifts byte offsets* -- text, audio, any stream where a
replacement can change buffer length -- the batch is sorted right-to-left
before application. A redaction near the end is applied first, so an
earlier insertion or deletion cannot invalidate the coordinates of later
redactions.

```
   buffer: [....a.....b.......c....]
                |     |       |
            entity1 entity2 entity3

   apply order: entity3, entity2, entity1
```

For codecs whose write step does *not* shift offsets -- image (bounding
boxes are independent), tabular (cell coordinates are stable) -- the
batch can be applied in any order, and the runtime may parallelize.
Right-to-left ordering is opt-in per modality, not a global invariant.
Each codec declares its own ordering contract. This avoids the failure
mode where a text-derived assumption silently mis-orders an image batch.

## 8. Audit Trail

Every entity reaching redaction produces an audit entry, regardless of
whether the redaction was applied. The audit is the authoritative log of
what happened to each piece of personal data; it is not a debug aid. Each
entry records two things:

- **The decision.** Which policy rule fired, whether an override applied
  (and which reviewer it came from, when available), what operator was
  selected, and which alternatives were considered. Answers *why* this
  entity was treated this way.
- **The execution.** Whether the redaction was applied, suppressed, or
  failed; for reversible operators, the metadata required to reverse it;
  for failures, the error and span coordinates. Answers *what happened*
  in the artifact.

The split matters because the sections have different lifetimes and
access controls. Decisions may need to be visible to compliance reviewers
without access to the artifact; executions, especially for reversible
operators, may need to be tightly held by key custodians. The audit is
append-only. A re-run produces a new audit, not an in-place update;
comparison between audits is how the runtime supports the question "what
changed between these two redactions of the same document".

## 9. Out of Scope

The runtime provides primitives. Several adjacent concerns belong to
deployments and are explicitly out of scope.

- **Differential privacy.** The catalogue does not include noise-addition
  operators tuned for differential-privacy guarantees. The supporting
  accounting -- privacy budgets, composition tracking, query auditing --
  is deployment-level.
- **k-anonymity / l-diversity guarantees.** The runtime does not analyze
  the corpus for re-identification risk after redaction. A k-anonymous
  output is the result of policy-authoring discipline, not a runtime
  guarantee.
- **External key management.** Reversible operators consume a key; they
  do not source it. Vault integration, HSM-backed signing, key rotation,
  and access policy are deployment integrations.
- **Reviewer UIs.** The runtime exposes an override surface, not a review
  interface. Presenting decisions to a human and collecting input is the
  responsibility of an external service.

## 10. Roadmap Limitations

Three areas are not yet at the conceptual completeness this paper
describes, noted so the model is not mistaken for the implementation.

- **Tabular redaction** is not yet a full phase with its own coordinate
  model. Tabular content is handled by the text pipeline over a serialized
  representation -- adequate for small structured payloads and inadequate
  for arbitrary tabular workloads.
- **Audio sample substitution** is partial. Silencing and removal are
  production-ready; substituting synthesized speech or alternate samples
  is supported only for a narrow set of inputs.
- **Operator policy composition** -- fallbacks such as "encrypt if a key
  is available, otherwise mask" -- is expressible but not first-class;
  deployments encode it as multiple rules.

These are roadmap items, not architectural limits. The operator and audit
boundaries are stable; what is missing is breadth of coverage inside the
existing categories.
