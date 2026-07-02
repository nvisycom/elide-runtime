# Vision-language OCR — why we use it

`nvisy-vl` (PaddleOCR-VL) is the vision-language OCR service. It reads a
document image and returns block-level **regions** — each with text, a layout
kind (text/table/figure/other), a bounding box, and a reading-order index. It is
the source of high-accuracy *text* the runtime uses to refine the detection-OCR
transcription.

## Why a VLM alongside traditional OCR

Traditional OCR ([docTR](tocr.md)) gives precise word-level **geometry**, but
its recognizer can misread:

- handwriting and low-quality scans,
- dense or complex multilingual text,
- unusual fonts and layouts.

A VLM is markedly better at *transcription accuracy* in those cases. So we run
both: the OCR service owns **geometry** (*where* the text is — pixel-precise
word boxes for masking), and the VLM owns **text content** (*what* the text
says). Geometry is never taken from the VLM — VLM coordinates are looser than
docTR's and aren't safe to bet mask precision on.

## Why PaddleOCR-VL

- **Per-region boxes.** Emits a 4-point quad + axis-aligned bbox per parsed
  block (`parsing_res_list`), in reading order — surfaced on the wire as
  `Region.bbox`. Good enough to *align* its text back to OCR regions, even
  though not precise enough to *replace* docTR's word geometry.
- **Layout + reading order.** Block-level layout labels (title/text/list/
  table/figure/chart/…), collapsed by the service into the contract's small
  `BlockKind`. Reading order falls out of the list order.
- **Compact (~0.9B), Apache-2.0.** Runs on a single GPU (or CPU, slowly) and
  has a clean license — the reasons it's the default VL backend.
- **Multilingual.** 109 languages out of the box, no per-language routing.

Contract:
[`nvisy_core.ocrvl.v1`](../../packages/nvisy-core/src/nvisy_core/ocrvl/v1.py).
Maps the engine's fine-grained labels onto the canonical `BlockKind` via the
service's
[`block_map`](../../packages/nvisy-vl/src/nvisy_vl/block_map.py).

## What it is *not*

- **Not a geometry source for masking.** Region boxes are useful for alignment,
  not for sub-line word masks — that stays with docTR.
- **Not a stand-alone OCR replacement.** Even with strong text, blindly
  trusting a VLM's transcription can *hurt* (VLMs hallucinate; see *"VLMs Are
  Not (Yet) Spelling Correctors"*, arXiv 2509.17418). The reconciliation policy
  — accept a VLM edit only when it's a bounded change to the OCR text, prefer
  running the VLM on low-confidence regions — lives in the runtime, not here.
- **Not always-on.** Whether to call the VL service is a per-request flag in
  the runtime; CPU-only deployments simply don't run this service at all.

## Why a separate, opt-in GPU service

A 0.9B+ VLM wants a GPU and busts the CPU footprint the OCR service targets.
Shipping the VLM as its own service gives independent scaling and failure
domains, and lets CPU-only deployments leave it out entirely. The runtime
orchestrates: it calls the OCR service for geometry+text and, when verification
is requested, the VLM service, then reconciles them.

## Swappability

The wire contract is ours, so any vision-language model that can fill `Region`
(text + layout kind + bbox + reading order) can replace PaddleOCR-VL without
touching the runtime — same `ocrvl.v1` contract, different image. Candidates
worth tracking:

- **Chandra OCR 2** — 5B (Qwen-based), strongest *text* in informal comparison
  (handwriting, forms, math, 90+ languages). Does produce per-region layout
  boxes (around figures/tables, surfaced via its `chunks` output). Not the
  default for two reasons that are decisive for a commercial product:
  **license** — weights are modified OpenRAIL-M (free only under ~$2M revenue,
  forbidden from competing with Datalab's API; quantized 2B/8B are
  commercial-only), and **footprint** — 5B, GPU-only (~2 pages/s on H100) vs.
  PaddleOCR-VL's 0.9B. A viable *premium opt-in* backend only if the license is
  cleared and the GPU cost is accepted.
- **Qwen3-VL** — general-purpose VLM with grounding; larger and slower for OCR,
  worth evaluating as the model landscape moves.

A formal SOTA review is tracked at
[issue #19](https://github.com/nvisycom/runtime/issues/297).
