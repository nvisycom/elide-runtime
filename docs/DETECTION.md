# Detection: A Conceptual Model

## Abstract

This paper describes the conceptual architecture of a system for identifying
sensitive data spans inside heterogeneous documents. The system is designed
around the observation that no single recognition technique is adequate across
all categories of personally identifiable information (PII), all content
encodings, and all confidence regimes. Multiple recognizer families run in
parallel against decomposed document fragments; their findings are lifted into
a shared coordinate system and reconciled through a deterministic
deduplication pipeline; annotations and policy rules then shape the surviving
candidates into the final detection artifact consumed by redaction.
Implementation details are out of scope.

## 1. The Detection Problem

Sensitive data detection is the task of locating spans of content that
constitute personally identifiable information within a document of arbitrary
structure. The problem is harder than it first appears because three axes vary
independently:

- **Category heterogeneity.** PII is not a single class. Email addresses,
  telephone numbers, government identifiers, financial account numbers,
  biometric references, medical history, geolocation traces, and free-form
  references to private third parties all qualify. Each category has its own
  surface form, its own jurisdictional variants, and its own contextual cues.

- **Encoding heterogeneity.** The same logical document can present its
  contents as plain prose, as cells in a spreadsheet, as text regions inside a
  raster image, as transcribed segments of an audio recording, as metadata
  attached to a binary asset, or as fragments embedded in a structured
  archive. A telephone number in a CSV cell is the same datum as a telephone
  number spoken in an interview, but the detection paths leading to them are
  not.

- **Confidence heterogeneity.** Different recognition techniques produce
  fundamentally different confidence semantics. A regular expression with a
  checksum is either correct or incorrect; the probability mass concentrates
  at the extremes. A neural sequence tagger emits a smooth distribution
  centered on a soft decision boundary. A generative model produces an answer
  whose calibration is opaque. Treating these signals as commensurable
  requires care.

A detection system that ignores any of these axes will fail in production.

## 2. Pluralistic Recognition

The central design commitment is pluralism: the system runs a population of
recognizers concurrently and treats each as a hypothesis-generator rather than
an authority. Three families dominate the population.

### 2.1 Rule-Based Recognition

Rule-based recognizers combine pattern matching with deterministic validators
and curated dictionaries. A regular expression establishes a candidate span; a
validator (checksum verification, structural conformance, range checking)
filters out coincidental matches; a dictionary lookup confirms membership in a
known set (country codes, common first names, area codes). Rule-based
recognition has high precision when the rule is well-formed, near-zero
latency, and very low recall outside the patterns it codifies. It cannot find
what it has not been told to look for.

### 2.2 Statistical Recognition

Statistical recognizers are sequence-labeling models trained on annotated
corpora. They consume extracted text and emit entity spans with associated
class probabilities. They generalize beyond a closed set of patterns and
capture entities whose surface form is irregular (person names, organization
names, free-form addresses) but their behavior is bounded by the distribution
of their training data. They are noisy near class boundaries and at the
edges of supported languages, and they are sensitive to domain shift.

### 2.3 Generative Recognition

Generative recognizers prompt a large language model to enumerate entities
within a fragment. They are valuable for categories that resist both rule
authorship and supervised training: open-class identifiers, paraphrased
references to sensitive attributes, contextual mentions ("the patient," "my
attorney"). They are the most expensive recognition path and the least
calibrated. Their utility is highest where the other families are weakest.

### 2.4 No Dominant Strategy

None of these families subsumes the others. Each has a precision-recall
profile shaped by its operating principle. The system does not elect a winner:
it runs the families in parallel and defers the question of which finding to
keep to a later stage that has access to all the evidence at once.

```
                          document
                              |
                  +-----------+-----------+
                  |           |           |
              rule-based  statistical  generative
              recognizer  recognizer   recognizer
                  |           |           |
                  +-----------+-----------+
                              |
                       candidate set
```

## 3. Chunking and Lifting

Documents are not scanned end-to-end. They are first decomposed into chunks:
paragraphs in prose, cells in tabular content, regions in images, segments in
audio. Chunking bounds the working set for any single recognizer invocation,
preserves locality so recognizer outputs remain interpretable, and exposes
structural context to the recognition layer (a tabular header can inform
interpretation of the cells beneath it).

Each recognizer reports its findings in coordinates local to the chunk it
inspected: a character offset within a paragraph, a pixel offset within an
image region, a millisecond offset within an audio segment. The system lifts
these local coordinates back into the source document's coordinate system.
Lifting is mechanical but essential: redaction acts on the original document
and must locate every finding inside it, irrespective of which chunk produced
the evidence.

## 4. Modality Boundaries

Recognition is organized by the *nature of the payload* presented to a
recognizer, not by the modality of the source document. Text recognizers
operate on textual payloads, regardless of whether the text came from a prose
paragraph, a spreadsheet cell, an OCR pass over an image region, or a
transcription pass over an audio segment. This factoring prevents
combinatorial duplication of recognizer logic across modalities and ensures
that improvements to a text recognizer benefit every upstream extraction
path.

The recognizer population is also open across modalities. Adding a native
image recognizer that emits bounding-box findings, or a native audio
recognizer that emits time-interval findings, does not perturb the existing
text recognizers; the new recognizer is registered alongside the existing
ones and contributes evidence on the same terms. The registry is a fan-out
point, not a closed taxonomy.

## 5. Deduplication and Conflict Resolution

The parallel evaluation of many recognizers against the same content produces
a redundant candidate set. The same entity is often discovered by multiple
recognizers; spans frequently overlap rather than coincide; class labels
sometimes disagree. A layered reconciliation pipeline reduces this to a
disjoint, deduplicated set of findings.

### 5.1 Threshold Filtering

The first layer applies a confidence floor. Candidates below a category- and
recognizer-specific threshold are discarded. This is not noise rejection in
the signal-processing sense; it is a policy decision about the precision-
recall tradeoff appropriate for the deployment. Thresholds are tunable and
auditable.

### 5.2 Overlap Resolution

The second layer resolves spatial conflicts. When two findings cover
overlapping spans (in text: overlapping character ranges; in images:
intersecting bounding boxes; in audio: overlapping time intervals), the
higher-confidence finding is preserved and the lower-confidence finding is
suppressed. Class agreement is not required: a generic name detection that
overlaps a more specific identifier detection yields to the latter.

### 5.3 Fusion

The third layer fuses *concurring* evidence. When multiple recognizers report
the same span and the same category, the system merges them into a single
finding whose confidence reflects the agreement of independent sources and
whose provenance records every contributing recognizer. Fusion is not a
simple maximum or average; it acknowledges that a rule-based hit and a
statistical hit on the same span are stronger evidence than either alone.

```
       raw candidate set
              |
              v
     +------------------+
     | threshold filter |  drop confidence < tau
     +------------------+
              |
              v
     +------------------+
     | overlap resolve  |  keep highest-confidence span
     +------------------+
              |
              v
     +------------------+
     |     fusion       |  merge concurring evidence
     +------------------+
              |
              v
       surviving entities
```

Order matters. Threshold filtering before overlap resolution prevents
low-confidence findings from displacing high-confidence ones. Overlap
resolution before fusion prevents fusing across categories. The pipeline is
deterministic given the same inputs.

## 6. Annotations: Ground Truth and Overrides

User-supplied annotations are first-class participants in detection, not a
separate workflow. They enter at two levels of authority.

A **hint** marks a span as likely sensitive, biasing detection toward
confirming the annotation but allowing recognizers to overrule it. Hints
amplify noisy upstream beliefs about sensitivity without dictating outcomes.

An **assertion** marks a span as known to contain a specific entity. It
materializes directly as a finding with maximum confidence, bypassing
recognizer evaluation for that span. Assertions are how authoritative
knowledge (a human reviewer's verdict, an audited upstream tag) enters
detection.

Symmetrically, **exclusions** mark spans that must not appear as findings.
Exclusions are applied as a final filter, removing any recognizer output
that falls within an excluded region. Exclusions are how users correct false
positives without retraining models or rewriting rules.

## 7. Policy Filtering

Detection identifies what is *present*. Policy decides what is *actionable*.
The separation is intentional. The set of entities in a document is a
property of the document; the set of entities subject to redaction in a
given workflow is a property of the workflow.

Policy is expressed as an ordered sequence of rules. Each rule conditions on
attributes of the candidate finding (category, recognizer confidence,
language, surrounding labels) and on attributes of the document (source,
classification labels attached by upstream systems, jurisdiction). The first
rule whose conditions are satisfied determines the disposition of the
candidate: retain, suppress, or annotate. Subsequent rules do not fire for
the same candidate.

This formulation has two consequences worth naming. First, the same
detection artifact can be filtered by different policies to produce
different actionable sets, supporting per-tenant or per-context redaction
without re-running detection. Second, policy authorship is auditable in
isolation: a reviewer can read the rule set and understand which entities
will be acted on without understanding how detection found them.

## 8. The Detection Artifact

The output of detection is an immutable record of every entity that survived
the pipeline. Each entry carries:

- a typed category;
- a position expressed in source-document coordinates;
- a confidence value derived from the contributing recognizers;
- a provenance trail naming every recognizer that contributed evidence and
  what evidence it contributed;
- any annotations attached by upstream systems or by the policy layer.

The artifact is the contract between detection and redaction. It is
serializable, inspectable, and replayable. A redaction pass consumes the
artifact and acts on the source document; it does not re-run detection. A
later audit can reconstruct exactly which recognizers fired on which spans
with which confidence, without access to the model weights or the rule
internals at the time of the original run.

The immutability of the artifact is what makes detection an *evidentiary*
operation rather than an opaque transformation. Every decision can be
attributed; every absence can be questioned.

## 9. Limitations and Honest Gaps

Detection accuracy is bounded by the accuracy of its constituent recognizers.
Rule-based recall is bounded by the rule set: an identifier format the rules
do not encode will not be found by rule. Statistical recall is bounded by
training data: an entity class underrepresented in the corpus will be
underdetected, and domain shift between training and deployment degrades
performance silently. Generative recall is bounded by the model: hallucinated
entities and missed entities are both possible, and the confidence the model
assigns to its own output is not necessarily informative.

The system does not promise zero leakage. It promises that detection is
pluralistic, that evidence is reconciled deterministically, that annotations
participate as first-class signals, that policy is separable from
recognition, and that the resulting artifact is auditable. These are
engineering invariants, not statistical guarantees. A deployment that
requires statistical guarantees must measure them on its own data and tune
its recognizer population, thresholds, and policy accordingly.

The architecture is open at every layer where recall can fail: recognizers
can be added, rules extended, models replaced, policies rewritten. The closed
surfaces are the ones that must be stable: the chunking and lifting model,
the deduplication pipeline, the artifact format. Stability in the contracts
is what allows pluralism in the recognizers.

## 10. Summary

Detection is the parallel evaluation of many fallible recognizers against a
chunked decomposition of a document, followed by deterministic reconciliation
of their findings, followed by a policy projection onto the actionable
subset. Annotations participate throughout. The product is an immutable,
attributable record of every entity the system believes is present. The model
is conservative about what it guarantees and generous about what it allows to
be extended.
