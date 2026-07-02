# Traditional OCR (docTR) — why we use it

`nvisy-ocr` (docTR) is the default OCR engine. It turns an image into a
`Page → Block → Line → Word` hierarchy with geometry, and is the source of the
text regions the runtime masks.

## Why a traditional (detection + recognition) OCR

Redaction has one hard requirement a pure end-to-end model struggles with:
**precise, word-level geometry**. To mask a *sub-line* span — e.g. just an SSN
inside a longer line — we need a tight box around that word, not the whole line.
NER gives character offsets; turning "this entity" into a pixel mask needs
per-word boxes. A detection+recognition pipeline is built to produce exactly
that.

## Why docTR specifically

- **Native word/line/block boxes.** docTR's two-stage (detection → recognition)
  design yields a real `Document → Page → Block → Line → Word` hierarchy with
  per-word geometry — which maps directly onto our contract. This is the precise
  geometry redaction depends on.
- **CPU-viable.** Runs without a GPU, keeping the OCR service within the
  self-hosted footprint.
- **Rotation support.** With `assume_straight_pages=False`, docTR returns 4-point
  polygons for skewed/rotated text; our mapping populates the contract's
  `polygon` field for that case.
- **Versus PaddleOCR.** The other obvious detection+recognition candidate is
  PaddleOCR (PP-OCRv5), but its detection is *line-level* — word boxes are
  derived post-rectification and degrade on rotation. docTR is strong exactly
  where PaddleOCR is weak for our use case.

## What it gives us

Per word: text, confidence, an axis-aligned pixel `BoundingBox`, and a `polygon`
when the region is rotated. Confidence is word-level only (docTR does not
aggregate to the line). Base docTR has no layout classification, so blocks
default to `BlockKind.TEXT`. Contract:
[`nvisy_core.ocr.v1`](../../packages/nvisy-core/src/nvisy_core/ocr/v1.py).

## What it is *not*

- **Not layout-aware (yet).** Base docTR detects text regions but does not
  classify blocks (title/table/figure) or reconstruct reading order. Richer
  structure would need a layout model (e.g. PP-StructureV3) — a future option.
- **Not a content corrector.** Classic OCR can misread messy/handwritten text;
  improving *accuracy* is the job of the optional VLM layer (see
  [`ocrvlm.md`](ocrvlm.md)). docTR owns *where*; the VLM owns *what*.

## Swappability

Geometry is always produced by this service; the wire contract is ours, so any
detection+recognition engine that fills `Page → Block → Line → Word` can replace
docTR without touching the runtime. Model selection is the `det+rec` pair via
`NVISY_MODEL_NAME`, with BYO weights via `NVISY_MODEL_PATH`.

A formal SOTA review is tracked at
[issue #18](https://github.com/nvisycom/runtime/issues/298).
