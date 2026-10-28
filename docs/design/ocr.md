# OCR: text with image geometry

`elide-ocr` turns an image into text and spatial regions. Redaction needs to
locate a sensitive substring on the source image, so the central design
requirement is word geometry that remains associated with recognized text.
See the [shared service architecture](README.md) for package and contract
boundaries.

## Model and output structure

The backend uses [docTR](https://github.com/mindee/doctr), a text detection and
recognition pipeline with a page/block/line/word hierarchy. That hierarchy maps
to the [OCR wire models](../../packages/elide-core/src/elide_core/ocr/v1.py) and
lets the runtime descend to the smallest available text region.

The default pair is `db_resnet50+crnn_vgg16_bn`, configured through
`ELIDE_BENTO_MODEL_NAME`. The engine constructs one predictor at startup and
reuses it. This separates the detection/recognition implementation from the
HTTP contract while retaining its useful geometry.

## Request and execution flow

`POST /recognize` takes a list under `requests`. Each item contains a base64
`image` and an optional `confidence_threshold`, defaulting to zero. The
[pipeline](../../packages/elide-ocr/src/elide_ocr/inference/pipeline.py) decodes
each image into RGB before passing it to the backend.

BentoML dispatches up to 16 images in a window, with a 15-second queue-latency
budget and 60-second request timeout. Within that window, the current pipeline
runs one image at a time. Dispatch batching therefore does not imply a docTR
model batch containing all images. A failure aborts the dispatched call; results
are not returned independently for each failed or successful item.

Malformed base64, unidentified images, known corrupt/truncated decoder errors,
and Pillow decompression-limit errors are input rejections. They increment
rejection metrics and become HTTP 400 responses. Operational failures such as
memory exhaustion and unavailable decoders propagate as server failures.

## Geometry and confidence

The [docTR projection](../../packages/elide-ocr/src/elide_ocr/inference/backends/doctr.py)
converts normalized coordinates to image pixels using the page's width and
height. Origin is top-left. A wire bounding box contains `x`, `y`, `width`, and
`height`; a polygon contains four two-dimensional points.

Confidence filtering is applied to words. Lines with no surviving words and
blocks with no surviving lines are removed. The surviving line and block boxes
are recomputed from their children so filtered text and geometry agree. Word
confidence is preserved; the current backend leaves line confidence unset and
assigns block kind `text`.

The projection accepts both two-corner boxes and four-point rotated geometry.
The shipped predictor uses docTR's default construction; it does not explicitly
enable rotated-page mode. Supporting polygons in the contract and projection
is separate from selecting a predictor configuration that emits them.

## Rust domain conversion

The [Rust adapter](../../crates/adapter/elide-bentoml/src/ocr/response.rs)
flattens the hierarchy into layout regions. It emits words when present, falls
back to a line when it has no words, and falls back to a block when it has no
lines. This preserves text from other conforming backends that report only
coarser regions.

Bounding boxes become minimum/maximum corners without renormalization. Positive,
one-based `page_number` values are preserved; values exceeding the domain's
`u32` range are rejected. The wire layer retains polygons, block kinds, page
dimensions, and model metadata. The current domain conversion uses axis-aligned
boxes and does not propagate those additional fields.

[Contract fixtures](../../contracts/ocr/v1) exercise pixel geometry, rotated
polygons, confidence, empty results, and both hierarchy fallback levels.

## Scope and extension points

The current backend recognizes text and provides geometry. It does not perform
semantic block classification, table reconstruction, or a second text-correction
pass. OCR errors therefore remain possible and can affect downstream detection.

A replacement backend must preserve the association between text and geometry.
It also needs compatible model loading and cache warming. Although settings
expose `ELIDE_BENTO_MODEL_PATH`, the current docTR loader passes the resolved
value through its `det+rec` constructor; arbitrary mounted weight directories
are not a complete custom-weight loading implementation.
