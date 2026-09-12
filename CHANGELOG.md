# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Nothing has been released yet, so this describes what the workspace
does today rather than how it got here.

### Crates

- **elide-pipeline** — the stateless engine and the umbrella entry
  point: decode, analyze, review, anonymize. Re-exports what the
  other crates define, so a host reaches everything from here.
- **elide-provider** — deployment-owned configuration: which NER, LLM,
  OCR and STT backends exist, and the per-modality orchestrator built
  from them. A request toggles them; it does not choose them.
- **elide-governance** — the wire schema for what happens to sensitive
  data: policies, scopes, rules, predicates, operator specs, and the
  vocabulary a request introduces.
- **elide-template** — ready-to-run policy postures for HIPAA
  §164.514, GDPR Article 9, PCI DSS §3.3.1/§3.5.1 and CCPA. Each
  carries its own version and the effective date of the text it
  encodes, independent of the crate release.
- **elide-review** — reviewer edits over a report: add, retag,
  suppress. Validated against the report before any of them land, so
  a set that cannot apply in full applies not at all.
- **elide-export** — audits as JSON documents or CSV tables.

### Detection and redaction

- One entity model across text, tabular data, images and audio, so an
  operator written once serves every modality.
- Layered detection: regex, dictionary and checksum patterns first,
  then NER, OCR, VLM and LLM recognition for what deterministic
  methods cannot reach. Language, OCR and speech enrichers feed the
  text they produce back through the same pipeline.
- Context-aware scoring, with overlapping findings reconciled into one
  deduplicated entity set.
- Operators: mask, replace, truncate, HMAC, hash, generalize and clamp
  text; blur, pixelate and black out image regions; silence and beep
  audio; drop tabular rows and columns. Encryption and pseudonymization
  round-trip.
- Codecs read and rewrite TXT, CSV, JSON, XML, HTML, RTF, PDF, DOCX,
  PPTX, XLSX, images (PNG, JPEG, TIFF) and audio (WAV, MP3), changing
  only the redacted spans. `codec-mp3` and `codec-pdf-render` stay
  opt-in: MP3 patent licensing may not be satisfiable downstream, and
  PDF rasterisation pulls in a native dependency.

### What a request supplies

- **Policies** say what happens to sensitive data: scopes naming the
  labels the policy acts on, rules within them, and a fallback for
  what no rule claimed.
- **`RequestContext`** says what the caller asserts about this
  document and this run — languages and jurisdictions, how the bytes
  decode, the key a keyed operator resolves through, and the
  vocabulary the request introduces beyond elide's shipped labels.
  Anything two callers sharing one engine could disagree about lives
  here rather than on the provider.
- A **`Recognition`** introduces labels elide does not ship, and
  matchers — a regex or a term list — that find them. A label without
  a matcher is legitimate: it joins the catalog for a reviewer to add
  entities under, and nothing detects it automatically.

### Auditing

- Every entity carries its full trail: how it was found, how it was
  scored, and how it was hidden. The trail verifies.
- An audit carries what the second pass needs and nothing it does not:
  the recognition context, the codec parameters and the request's
  vocabulary, so `anonymize` compiles against the same labels and
  decodes to the same bytes `analyze` did. The key is never recorded.
- Detections no policy acted on are reported rather than left silent.

### Inference

- Model services live in
  [nvisycom/elide-provider](https://github.com/nvisycom/elide-provider)
  and are reached over HTTP. The engine ships clients for them, and
  any service reproducing the wire contract is a drop-in replacement,
  including self-hosted models and weights.

[Unreleased]: https://github.com/nvisycom/elide-runtime/commits/main
