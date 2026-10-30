# OCR: text with image geometry

`elide-ocr` turns an image into text and spatial regions. Redaction needs to
locate a sensitive substring on the source image, so the central design
requirement is word geometry that remains associated with recognized text.
See the [shared service architecture](README.md) for package and contract
boundaries.

## Choosing the model

### What the service needs

Beyond [the bar every model meets](README.md#what-every-model-must-meet):

- **Word-level geometry.** The runtime blacks out the pixels where a sensitive
  word was printed, so each recognized word needs its own box, kept with its
  text. Page-level text, or lines without word boxes, cannot be redacted
  precisely.
- **Every character of a name.** A name read with one letter wrong does not
  match, and a letter outside the model's vocabulary can never be read at all.
  For redaction, a model's vocabulary is a hard limit on what it can protect.
- **Faithful text.** The output has to be what is printed. A model that
  corrects, normalizes, or fills in plausible text breaks the match between
  text and location.
- **Scans as they arrive.** Low resolution, blur, compression, noise, slight
  skew, faint ink.
- **A small image.** OCR is the service most likely to be scaled out, so its
  image carries no PyTorch.

### Options considered

| Option | Licence | Scripts | Verdict |
|---|---|---|---|
| **OnnxTR, multilingual `parseq`** | Apache-2.0 | Latin, 12 European languages | **Chosen.** Word geometry, the best measured accuracy on personal data, no PyTorch |
| OnnxTR, `crnn_vgg16_bn` (the previous default) | Apache-2.0 | Latin, French vocabulary | Faster, but cannot read `á`, `ä`, `ł`, or `Ł`; see the evaluation below |
| OnnxTR, `parseq` (built-in) | Apache-2.0 | Latin, French vocabulary | The same vocabulary limit, and no more accurate than `crnn_vgg16_bn` here |
| docTR on PyTorch | Apache-2.0 | Same models | The same models in an image about twice the size (2.15 GB against 1.08 GB) |
| PaddleOCR (PP-OCRv5) | Apache-2.0 | Many, Cyrillic included | The strongest candidate for scripts beyond Latin; needs the Paddle runtime or an ONNX export, and has not been evaluated here |
| Tesseract | Apache-2.0 | 100+ | Word boxes and broad language packs; not evaluated here |
| EasyOCR | Apache-2.0 | 80+ | Needs PyTorch; not evaluated here |
| Surya | Apache-2.0 code, OpenRAIL-M weights | 90+ | The weights' licence is free only below $5M in funding or revenue |
| Vision-language models | Various | Many | No reliable word boxes, and they can produce text that is not on the page |
| Hosted ([Textract](../../crates/adapter/elide-aws-textract), [Document AI](../../crates/adapter/elide-google-document-ai)) | Service terms | Many | The image leaves the deployment; available as adapters for operators who accept that |

### Why OnnxTR's multilingual `parseq`

[OnnxTR](https://github.com/felixdittrich92/OnnxTR) runs
[docTR](https://github.com/mindee/doctr)'s detection and recognition models on
ONNX Runtime, so the image carries no PyTorch. It keeps docTR's
page/block/line/word hierarchy with a box for every word, which maps onto the
[OCR wire models](../../packages/elide-core/src/elide_core/ocr/v1.py) directly.

Detection is `db_resnet50`, a built-in. For recognition, every built-in model
has a French vocabulary, without `á`, `ä`, `ł`, or `Ł`, so names such as García,
Schäfer, Nováková, or Łukasz are unreadable however clean the scan. The default
is docTR's multilingual `parseq`, `Felix92/onnxtr-parseq-multilingual-v1` at a
pinned commit (Apache-2.0, published by OnnxTR's maintainer). Its vocabulary
covers English, Danish, French, Italian, Spanish, German, Portuguese, Czech,
Polish, Dutch, Norwegian, and Finnish.

It was compared with the two strongest built-ins on synthetic A4 pages at
200 dpi with exact ground truth: five fonts, each under seven conditions
(clean, 150 dpi, Gaussian blur, JPEG at quality 30, salt-and-pepper noise, a
1° rotation, faint ink), with `db_resnet50` detection throughout. A
personal-data token counts as read only if every one of its words came back
exactly, which is what a redaction match needs.

| Recognizer | Word recall | Character error | Personal-data tokens read |
|---|---|---|---|
| `crnn_vgg16_bn` | 0.930 | 0.020 | 0.755 |
| `parseq` (French) | 0.932 | 0.020 | 0.762 |
| multilingual `parseq` | 0.949 | 0.017 | 0.889 |

Means over the seven conditions. On clean pages the multilingual model read
every name (the built-ins 0.62) and 0.91 of email addresses (0.41 and 0.50).
It is weaker in three places: IBANs (0.72 against 0.80), amounts (0.93 against
0.98), and heavy salt-and-pepper noise (word recall 0.890 against 0.913). It is
also slower: about 12 seconds per dense page on CPU against 7 for
`crnn_vgg16_bn`. Synthetic pages are no substitute for a deployment's own
documents, and this should be measured again against real scans.

### Known gaps

- **Latin script only.** No Cyrillic, Greek, or non-European scripts, and no
  Hungarian `ő`. A document in those scripts is read partly or not at all.
- **Weaker on some formats.** IBANs and amounts are read less reliably than by
  `crnn_vgg16_bn`, and heavy salt-and-pepper noise costs more recall.
- **Slower.** About 1.7× `crnn_vgg16_bn` on CPU.
- **Synthetic evaluation.** Generated pages, not a deployment's real scans.
- **Detection not compared.** `db_resnet50` was held fixed; the other detection
  architectures were not measured.
- **Straight pages assumed.** The predictor does not correct page orientation;
  see [Geometry and confidence](#geometry-and-confidence).

### When to revisit

- Documents in Cyrillic or Greek: PaddleOCR is the first candidate to evaluate.
- A real-scan evaluation that shows the IBAN and amount weakness matters, which
  could justify a format-specific second pass.
- A GPU deployment, where the speed difference stops mattering and larger
  recognizers become affordable.
- A newer multilingual docTR release.

## How the service runs it

The pair is `ELIDE_BENTO_OCR_DETECTION` and `ELIDE_BENTO_OCR_RECOGNITION`. The
engine constructs one predictor at startup and reuses it.

Each model is named one of two ways, and both pin the weights:

- **A built-in OnnxTR architecture**, such as `db_resnet50`. Its weights are
  OnnxTR's own release assets, and each checkpoint's filename carries a SHA-256
  prefix that OnnxTR checks on download and on every cache hit, so the `onnxtr`
  version in the lockfile pins them. They are cached in `ONNXTR_CACHE_DIR`.
- **A Hugging Face Hub model pinned to a full commit**,
  `<owner>/<repo>@<40-hex sha>`, loaded through OnnxTR's `from_hub`. A branch,
  a tag, or a short sha is rejected at startup: each can name different weights
  tomorrow, and a commit cannot. They are cached under `HF_HOME`, and the service
  runs with `HF_HUB_OFFLINE=1`, so a pinned commit resolves from disk.

The warmer builds the service's own engine, which is OnnxTR's only public way to
populate either cache and guarantees it fetches exactly what the service loads.

`ELIDE_BENTO_OCR_DEVICE` selects the ONNX Runtime execution provider, `cpu` or
`cuda`. The providers are always named explicitly: left to choose, OnnxTR
prefers CoreML where it exists, and CoreML cannot build `parseq`. `cuda` needs
an image built with `onnxruntime-gpu`, which cannot share an environment with
the CPU `onnxruntime`. Without it, startup fails rather than letting ONNX Runtime
fall back to CPU unannounced.

### Request and execution flow

`POST /recognize` takes a list under `requests`. Each item contains a base64
`image` and an optional `confidence_threshold`, defaulting to zero. The
[pipeline](../../packages/elide-ocr/src/elide_ocr/inference/pipeline.py) decodes
each image into RGB before passing it to the backend.

BentoML dispatches up to 3 images in a window, with a 15-second queue-latency
budget and 60-second request timeout. Within that window, the current pipeline
runs one image at a time. Dispatch batching therefore does not imply an OnnxTR
model batch containing all images, and deliberately so: on CPU, one predictor
call over a window is slower than a call per image. Measured over 16 A4 pages at
200 dpi (`db_resnet50+crnn_vgg16_bn`, identical output either way), one call took
1.9–2.4× as long and peaked 0.7–1.7 GiB higher, whatever the detection or
recognition batch size. ONNX Runtime already spreads a single page across every
core, so a larger batch adds tensor size, not parallelism. A GPU image would need
measuring again. A failure aborts the dispatched call; results are not returned
independently for each failed or successful item.

Because the images in a window run one after another, a larger window adds
waiting, not throughput, and the window is sized to the timeout instead: 15
seconds queued plus three dense pages at about 12 seconds each is 51 seconds. A
slower host or denser pages need a smaller window or a longer timeout. A caller
sending more pages than fit in one request splits them across several, as the
[Rust adapter](../../crates/adapter/elide-bentoml/src/ocr/mod.rs) documents.

Malformed base64, unidentified images, known corrupt/truncated decoder errors,
and Pillow decompression-limit errors are input rejections. They increment
rejection metrics and become HTTP 400 responses. Operational failures such as
memory exhaustion and unavailable decoders propagate as server failures.

### Geometry and confidence

The [OnnxTR projection](../../packages/elide-ocr/src/elide_ocr/inference/backends/onnxtr.py)
converts normalized coordinates to image pixels using the page's width and
height. Origin is top-left. A wire bounding box contains `x`, `y`, `width`, and
`height`; a polygon contains four two-dimensional points.

Confidence filtering is applied to words. Lines with no surviving words and
blocks with no surviving lines are removed. The surviving line and block boxes
are recomputed from their children so filtered text and geometry agree. Word
confidence is preserved; the current backend leaves line confidence unset and
assigns block kind `text`.

The projection accepts both two-corner boxes and four-point rotated geometry.
The shipped predictor uses OnnxTR's default construction; it does not explicitly
enable rotated-page mode. Supporting polygons in the contract and projection
is separate from selecting a predictor configuration that emits them.

### Rust domain conversion

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

### Scope and extension points

The current backend recognizes text and provides geometry. It does not perform
semantic block classification, table reconstruction, or a second text-correction
pass. OCR errors therefore remain possible and can affect downstream detection.

A replacement backend must preserve the association between text and geometry.
It also needs compatible model loading and cache warming. Custom weights are
loaded the same way as the default recognizer: published as an OnnxTR model on
the Hub and named at a pinned commit. A local path is not accepted, so every
deployment's weights trace back to an immutable, published revision.
