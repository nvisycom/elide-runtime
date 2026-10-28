# OCR v1

Python's `elide_core.ocr.v1` owns this contract. See the [generation workflow](../..).
`OcrInput` is `{"requests": [...]}`; `OcrOutput` is an ordered response array.

Bounding boxes use image pixels and top-left origin. Polygons contain exactly
four `[x, y]` points. Pages use positive, one-based indices. The Rust adapter
preserves text and page indices while flattening pages/blocks/lines/words into
regions. It emits words where available, falling back to lines and then blocks.
Boxes become minimum/maximum corners without normalization. Page indices that
cannot fit the Rust domain's `u32` are rejected rather than truncated.

Generated wire types retain polygons, block kinds, page dimensions, and model
metadata. The current Rust domain conversion continues to omit those fields.
Fixtures cover rotated geometry, word confidence, both fallback levels, and an
empty result. Structural validation is generated; field limits remain enforced
by the Python service.
