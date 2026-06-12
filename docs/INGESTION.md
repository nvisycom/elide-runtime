# Document Ingestion in a Multimodal PII System

## Abstract

A privacy system is, before anything else, a decoder. Detection models and
redaction policies operate on structured representations of content; the world
delivers content as opaque byte streams in dozens of incompatible formats.
Ingestion is the layer that bridges the two: it accepts heterogeneous bytes,
resolves them to a typed representation that detection can address, mediates
the mutation that redaction performs, and re-emits bytes in the original format
without losing fidelity. This paper describes the conceptual model that governs
ingestion in our runtime. We treat formats not as a catalogue but as instances
of a single abstraction — the typed handle — and we describe the coordinate
systems, mutation contract, and round-trip guarantees that hold across the
abstraction.

## 1. The Ingestion Problem

Every input to a privacy system arrives as a sequence of bytes paired with a
claim about what those bytes mean. The system must produce, from that pair, a
representation against which detection can operate and into which redaction
can write. The difficulty is that no two formats agree on what either of those
operations means.

A plain-text file encoded in UTF-8 differs from the same text in UTF-16 not
just at the byte level but in the unit of indexing a recognizer must use.
A structured document hides its strings behind escape sequences, so the text a
recognizer sees is not the text the file contains. Markup languages resolve
entity references and decode attribute values, presenting the recognizer with
a view that may have no direct byte correspondence to the source. Delimited
text formats negotiate quoting rules whose details depend on a dialect. Images
speak in pixel coordinates; audio speaks in time. Each format also disagrees on
what counts as a redactable unit: a span of characters, a scalar value, a
rectangle in pixel space, a half-open interval of audio samples.

Ingestion is the discipline of generalising over these formats without erasing
them. A naive approach would coerce everything into one common type — say, a
string of characters — and lose the formats on the way in. The result would be
a system that detects names in a word-processing document but cannot produce a
word-processing document back. A correct ingestion layer keeps the format
present from the moment bytes arrive to the moment bytes leave.

## 2. The Content Bundle

Content does not arrive as a single object. It arrives as several orthogonal
facets that the system keeps separate on purpose:

```
                            content
                               |
        +----------+-----------+-----------+----------+
        |          |           |           |          |
      bytes     descriptor   digest      record    (future)
        |          |           |           |
      raw       caller-       system-    system-
      data      supplied      computed   derived
      +         metadata      metadata   metadata
      stable    (claimed      (hashes    (storage
      source    format,       for        time,
      id        filename,     dedup,     derived
                source path)  integrity) attributes)
```

The split is not cosmetic. Each facet has a different trust level and a
different lifecycle. Raw bytes are immutable: they are what was uploaded.
Descriptor metadata is caller-supplied and may be wrong — by accident or
otherwise. Digest metadata is computed by the system from the bytes themselves
and is authoritative. Record metadata captures processing facts the system
knows but the caller could not have known — when the content entered the
system, what derived attributes the pipeline later attached.

Merging these facets would corrupt them. A caller who misreports a format
could overwrite a system-computed integrity hash; a system-derived attribute
could be confused with caller intent. Keeping the facets separate keeps the
trust boundaries legible: at any later stage, the pipeline can ask whether a
fact came from the caller or from the system, and treat it accordingly.

## 3. Format Detection

Detection is the act of choosing which decoder to apply to a stream of bytes.
The system resolves format identity by consulting signals in priority order:

```
   caller MIME hint  ->  filename extension  ->  content-type heuristics
        (1)                    (2)                       (3)
```

The caller's explicit hint takes priority. If the upload arrived through a
request with a content-type header, or through a programmatic interface that
named the format, that claim wins. When no hint exists, the filename
extension is consulted. When neither is available, a small set of
content-derived heuristics fills the gap.

The runtime does not perform deep magic-byte sniffing. This is a deliberate
design choice with a real cost. Deployments that need probabilistic format
detection — accepting uploads from end users who provide no metadata, for
instance — must add a sniffer upstream. Inside the runtime, format identity
is treated as supplied: the system decodes according to what it was told.
The tradeoff is precision for predictability. A runtime that re-identifies
formats can disagree with the caller about what was uploaded, and that
disagreement becomes a class of bug that does not exist when the caller is
authoritative.

## 4. The Typed Handle

Once decoded, content is no longer a byte stream; it is a typed handle whose
type encodes both the format and the modality. The handle is the central
abstraction of the ingestion layer, and every downstream stage operates
through it.

A handle exposes five capabilities:

1. **Streaming chunking.** The handle yields redactable units in document
   order. A unit may be a span of text, a value in a structured document, a
   region of an image, a segment of audio. The chunker streams units as the
   recognizer requests them, without materialising the entire decoded view.
2. **Random-access retrieval.** Given a coordinate, the handle returns the
   unit at that coordinate, supporting stages that revisit units out of
   document order.
3. **In-place mutation.** Redaction does not rewrite the handle from scratch;
   it mutates the units the recognizer identified. Mutation is addressed by
   coordinate and confined to the units the policy named.
4. **Re-encoding.** The handle serialises itself back to bytes in the same
   format as the input.
5. **Coordinate lifting.** Given an offset in the decoded view a recognizer
   saw, the handle returns the corresponding offset in the source bytes.

The handle's type is parameterised on the modality. Downstream code that
holds a handle does not need to ask what kind of content it carries; the
type already encodes it, and a routine written for text handles cannot
accept an image handle by mistake.

## 5. Modality Boundaries Within a Document

Some formats produce a homogeneous decoded view: a plain-text file decodes to
text, an image file decodes to an image, an audio file decodes to audio. Many
real-world formats do not. A word-processing document combines text with
embedded images and inline comments. A markup page mixes textual body content,
attribute values, embedded scripts, and structural markup, each with its own
redaction semantics.

The system models a mixed handle as a heterogeneous stream of redactable
items. Each item carries with it both the kind of unit it represents and the
knowledge of how to write a mutated value back at encode time. A text span
knows how to substitute its replacement into the appropriate container slot;
an image region knows how to overwrite its pixel range; an attribute value
knows how to escape itself back into a markup attribute. The recognizer
addresses items by kind; the encoder delegates to each item's own write-back
logic.

Modality is a per-item property, not a per-document property. A single handle
can yield a text item, then an image item, then another text item, in
document order, and each is handled by the recognizer appropriate to its
modality.

## 6. The Decode-Redact-Encode Loop

The lifecycle of content inside the ingestion layer is a single loop:

```
   bytes
     |
     v
   decode  --->  typed handle  --->  chunker  ---+
                       ^                          |
                       |                       items
                       |                          |
                   mutations                      v
                       |                     recognizer
                       |                          |
                       |                       findings
                       |                          |
                       |                          v
                       |                       lift to
                       |                       source
                       |                       coords
                       |                          |
                       +--------------------------+
                       |
                       v
                   re-encode
                       |
                       v
                    bytes
```

The handle is decoded once. The chunker streams items into the recognizer,
which identifies sensitive regions in the decoded view. The lifter translates
those regions from decoded coordinates back to source coordinates. The policy
stage decides what to do with each finding, and mutations are applied to the
handle in source coordinates. The encoder serialises the mutated handle back
to format-native bytes. There is no second decode pass and no separate write
phase: the handle is the medium through which decoding, recognition, and
re-encoding communicate.

## 7. The Encode Round-Trip Contract

Re-encoding has a two-clause contract.

First, untouched units serialise byte-for-byte where the format permits. For
formats whose parsers preserve a verbatim view of the source — plain text and
structured formats where unmodified regions are kept as raw slices — the
encoder copies untouched bytes directly. The output of a document with no
mutations is byte-identical to the input.

Second, touched units serialise their mutated value through format-aware
write-back. A text replacement is escaped according to the format's rules. A
redacted image region is composited into the output pixel buffer. An audio
segment is replaced in the sample stream.

Some formats cannot satisfy the first clause completely. Markup languages and
rich word-processing formats discard structural information during parsing —
attribute ordering, whitespace between tags, default values left implicit.
For these formats, re-encoding necessarily re-parses and re-serialises the
source, producing output that is semantically equivalent but not byte-identical
to the input. The system makes this tradeoff explicit per format rather than
silently degrading round-trip fidelity across the board.

## 8. Coordinate Systems and Lifting

The recognizer and the encoder do not speak the same coordinate language. A
recognizer sees the decoded payload of a chunk: escape sequences resolved,
percent-encoding decoded, content extracted from containers, character
encodings normalised. Its offsets index into that decoded view. The encoder
must address the source. A mutation expressed in decoded coordinates would
land on the wrong bytes — sometimes by a few characters, sometimes by an
entire escape sequence, sometimes by a chunk boundary.

The lifting contract closes this gap. Given a decoded offset, the handle
returns the corresponding source offset. The mechanism varies by format:

```
   format class                lifting mechanism
   --------------------------+-----------------------------------------
   verbatim text             | identity (decoded offset == source)
   escaped textual formats   | escape map walked at lift time
   container/parsed formats  | item-index coordinates, not byte offsets
```

For verbatim text, the lift is the identity function. For formats with escape
sequences, the handle maintains an escape map built during decode and walks
it at lift time. For formats whose parsers discard byte-level structure, the
lift cannot return a meaningful source offset; the system accepts that
source-byte fidelity is lost and addresses items by structural index instead.

The lifting contract allows recognizers to be format-agnostic. A recognizer
for personal names does not need to know about escape sequences or entity
references; it operates on decoded text and returns decoded offsets, and the
handle is responsible for translating those into something the encoder can
act on.

## 9. Compression and Encryption Boundaries

Some ingestion paths receive bytes that are not in their final form. Content
may be compressed at rest or encrypted with a symmetric scheme. The pipeline
decompresses and decrypts before decode, performs the decode-redact-encode
loop on the cleartext, and may re-encrypt and re-compress before storage.

The runtime does not hold long-lived keys. Keys are supplied by a
deployment-pluggable key provider, which the runtime calls at the moment a
cleartext is needed. The provider can be backed by a hardware security
module, a managed key service, or a process-local secret, as the deployment
requires. The runtime sees cleartext only for the duration of the loop.

This boundary separates two threats. Threats against the keys are addressed
by the provider's deployment. Threats against the cleartext during processing
are addressed by the runtime's own memory hygiene. Conflating the two — for
instance, by caching decrypted bytes in the runtime — would weaken both.

## 10. Persistence and Retrieval

Ingested content is persisted in a registry. The registry key is the source
identifier paired with an actor scope, which provides multi-tenant isolation:
two tenants may upload content with the same source identifier without
colliding.

Persistence stores the bundle's facets separately, matching the in-memory
split. Retrieval reconstructs the bundle by reading each facet and
reassembling them, so downstream stages receive the same bundle they would
have received on first ingest. A pipeline that crashes mid-processing can
restart from the persisted bundle; a re-detection after a policy update can
read the bundle without asking the caller to re-upload; audit replays can
reconstruct the exact bytes the pipeline processed.

## 11. Honest Scope

The runtime supports a curated set of formats today. Some are full
implementations with bidirectional decode-encode fidelity; some are present
as stubs whose interfaces are stable but whose internals are incomplete. The
architecture treats new format support as a plug-in concern: adding a format
means implementing a decoder that produces a handle and an encoder that
serialises one. Once both exist, the format is available to detection, to
redaction, to persistence, and to every downstream stage without further
changes.

New formats require code. They do not require pipeline changes, schema
migrations, or coordination across stages. This is the property the
ingestion abstraction was designed to provide, and it is the property by
which the abstraction should be judged.
