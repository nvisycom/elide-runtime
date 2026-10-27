"""OCR inference service (docTR) exposed over HTTP via BentoML.

The default implementation of the OCR wire contract (``elide_core.ocr.v1``).
docTR natively produces a ``Document -> Page -> Block -> Line -> Word`` hierarchy
with word-level geometry, which maps directly onto our contract.

Run locally::

    uv run bentoml serve elide_ocr.service:OcrService --reload
"""

from __future__ import annotations

import base64
import binascii
import io
import os
import time
from typing import TYPE_CHECKING

import bentoml
from bentoml.exceptions import BentoMLException, InternalServerError, InvalidArgument
from elide_core.ocr.v1 import (
    Block,
    BlockKind,
    BoundingBox,
    Line,
    OcrRequest,
    OcrResponse,
    Page,
    Polygon,
    Word,
)
from elide_core.runtime import get_logger, request_id, resolve_model
from prometheus_client import Counter, Histogram

if TYPE_CHECKING:
    from PIL.Image import Image as PILImage

logger = get_logger("nvisy.ocr")

# Built-in default detection + recognition models, joined as "det+rec".
# Declared as the ELIDE_BENTO_MODEL_NAME env default below (single source of truth).
DEFAULT_MODEL = "db_resnet50+crnn_vgg16_bn"

batch_size_metric = Histogram(
    "elide_ocr_batch_size",
    "Number of images merged into one recognize() call.",
    buckets=(1, 2, 4, 8, 16),
)

# Which bound a refused request breached. BentoML already counts 400s; what it
# cannot say is whether the caller sent corrupt base64 or bytes that are not an
# image, and those point at different faults upstream.
rejected_metric = Counter(
    "elide_ocr_rejected_total",
    "Image payloads refused before inference, by which bound they breached.",
    labelnames=("reason",),
)

# Words returned per image. Zero across a batch is the signature of a blank or
# unreadable scan rather than a quiet success.
words_metric = Histogram(
    "elide_ocr_words_found",
    "Words returned per image.",
    buckets=(0, 1, 8, 32, 128, 512, 2048),
)

# Build-time image spec, skipped inside the container. `requirements_file`
# reads the file when this module is imported, and the path it needs differs
# between the two contexts (`packages/...` at build time, `src/packages/...`
# under the bento root), so evaluating it at runtime raises FileNotFoundError
# and the service never starts. BENTOML_CONTAINERIZED is set in the image and
# unset during a build, which is the distinction.
#
# build_include copies the bundled workspace source to ./src *before* the
# image's `uv pip install`, which is what makes the editable path dependencies
# in requirements.txt resolvable: the install then runs with
# INSTALL_ROOT=./src, where `./packages/elide-*` are real directories.
#
# lock_python_packages=False: the file is already locked and hashed by
# scripts/gen_requirements.py, so BentoML must not re-resolve it.
image = (
    None
    if os.getenv("BENTOML_CONTAINERIZED")
    else (
        bentoml.images.Image(python_version="3.12", lock_python_packages=False)
        .system_packages("libgl1", "libglib2.0-0")
        # The torch pins are `+cpu` builds from the PyTorch CPU index, which
        # requirements.txt carries as an --extra-index-url. That index also mirrors
        # unrelated packages (certifi, filelock, ...) at versions that may not match
        # ours, and uv defaults to `first-index`: it refuses to look past the first
        # index that carries a name at all, so the install fails on those.
        #
        # `--index-strategy` is not a directive a requirements file accepts, and a
        # RUN cannot export an env var to the next layer, so the setting goes in
        # uv's system config, which uv reads with no further configuration. This
        # runs before the install step.
        #
        # "unsafe" describes resolution, not installation: every pin in
        # requirements.txt is hash-locked, so an artifact from any index is
        # rejected unless its SHA-256 matches the lockfile. The hashes, not the
        # index order, are what make this safe.
        .run(
            "mkdir -p /etc/uv && printf 'index-strategy = \"unsafe-best-match\"\\n'"
            " > /etc/uv/uv.toml"
        )
        .build_include("packages", "pyproject.toml", "uv.lock")
        .requirements_file("packages/elide-ocr/requirements.txt")
    )
)


@bentoml.service(
    name="elide-ocr",
    image=image,
    resources={"cpu": "2"},
    traffic={"timeout": 60},
    # Declared with defaults so they're optional + documented in the bento
    # manifest. ELIDE_BENTO_MODEL_PATH defaults to the /models mount (empty unless BYO
    # weights are mounted); ELIDE_BENTO_MODEL_NAME is the "det+rec" pair otherwise.
    envs=[
        {"name": "ELIDE_BENTO_MODEL_PATH", "value": "/models"},
        {"name": "ELIDE_BENTO_MODEL_NAME", "value": DEFAULT_MODEL},
    ],
)
class OcrService:
    def __init__(self) -> None:
        from doctr.models import ocr_predictor

        self.model_id = resolve_model()
        det, _, rec = self.model_id.partition("+")
        if not rec:
            raise RuntimeError(f"ELIDE_BENTO_MODEL_NAME must be 'det+rec', got {self.model_id!r}")
        logger.info("loading docTR (det=%s, rec=%s)", det, rec)
        started = time.monotonic()
        try:
            self.ocr = ocr_predictor(det, rec, pretrained=True)
        except Exception as exc:
            # Log the cause here: the traceback a container log keeps is often
            # only its last lines, and docTR downloads weights during this
            # call, so the useful detail (a network or cache failure) is where
            # it is raised rather than where it surfaces.
            logger.exception("failed to load docTR (det=%s, rec=%s): %s", det, rec, exc)
            raise RuntimeError(f"failed to load docTR {self.model_id!r}: {exc}") from exc
        # Elapsed, because a cold start fetches weights and the difference
        # between "slow" and "wedged" is otherwise unknowable from the log.
        logger.info("docTR ready in %.1fs", time.monotonic() - started)

    # Sync (not async): inference is CPU/GPU-bound and blocking. BentoML runs
    # sync endpoints in a managed thread pool, so this never blocks the event
    # loop (an async def here would, and could starve /readyz).
    # max_batch_size is the dispatcher's window: a longer call is split into
    # windows of this size and the results rejoined. 16, half of elide-ner's,
    # because `recognize` runs `_recognize_one` sequentially and a page is far
    # more work than a sentence, so a window here is a much longer extraction.
    #
    # max_latency_ms is a deadline on queued work, and the old 120ms was far
    # shorter than one page, so concurrent callers were cancelled with `503
    # process is overloaded` rather than waited for — losing a detection over
    # a few seconds of queueing. 15s is not tuned to a measurement: it is long
    # enough that queueing never fails, and under traffic.timeout so a stuck
    # queue is cut off by the request timeout instead. See elide-ner's note.
    @bentoml.api(batchable=True, max_batch_size=16, max_latency_ms=15000)
    def recognize(
        self,
        requests: list[OcrRequest],
        ctx: bentoml.Context,
    ) -> list[OcrResponse]:
        batch_size_metric.observe(len(requests))
        rid = request_id(ctx)
        logger.info("recognize batch=%d req_id=%s", len(requests), rid)
        started = time.monotonic()
        try:
            responses = [self._recognize_one(req, rid) for req in requests]
        except BentoMLException:
            # Typed request errors (e.g. InvalidArgument for bad image bytes)
            # carry their own status; let them through unchanged.
            raise
        except Exception as exc:
            logger.exception("inference failed (req_id=%s)", rid)
            raise InternalServerError("OCR inference failed") from exc

        # One line per call carrying what the metrics summarize, so a request
        # can be explained from the log alone. Zero words across a batch means
        # a blank or unreadable scan, which is otherwise indistinguishable
        # from a quiet success.
        words = 0
        for resp in responses:
            found = sum(
                len(line.words)
                for page in resp.pages
                for block in page.blocks
                for line in block.lines
            )
            words_metric.observe(found)
            words += found
        logger.info(
            "recognized batch=%d words=%d elapsed=%.3fs req_id=%s",
            len(requests),
            words,
            time.monotonic() - started,
            rid,
        )
        return responses

    def _recognize_one(self, req: OcrRequest, rid: str | None = None) -> OcrResponse:
        image = _decode_image(req.image, rid)
        result = self.ocr([_to_ndarray(image)])
        # We pass one image, so expect one page; guard an empty result instead
        # of indexing into nothing.
        pages = [
            _page_from_doctr(p, req.confidence_threshold) for p in getattr(result, "pages", [])
        ]
        return OcrResponse(pages=pages, model_id=self.model_id)


def _decode_image(image_b64: str, rid: str | None = None) -> PILImage:
    from PIL import Image, UnidentifiedImageError

    try:
        raw = base64.b64decode(image_b64, validate=True)
    except (binascii.Error, ValueError) as exc:
        rejected_metric.labels(reason="base64").inc()
        logger.warning("rejected: image is not valid base64 (req_id=%s)", rid)
        raise InvalidArgument("image is not valid base64") from exc
    try:
        return Image.open(io.BytesIO(raw)).convert("RGB")
    except UnidentifiedImageError as exc:
        rejected_metric.labels(reason="not_an_image").inc()
        logger.warning("rejected: image bytes are not a recognizable image (req_id=%s)", rid)
        raise InvalidArgument("image bytes are not a recognizable image") from exc


def _to_ndarray(image: PILImage):
    import numpy as np

    return np.asarray(image)


def _page_from_doctr(page, threshold: float) -> Page:
    """Map a docTR page onto the contract.

    docTR geometry is normalized [0, 1] relative to the page; ``dimensions`` is
    ``(height, width)`` in pixels. We scale geometry to pixels and keep the full
    Block -> Line -> Word hierarchy docTR produces. Base docTR has no layout
    classification, so blocks default to BlockKind.TEXT.

    Confidence is only available at word level (docTR), so Line/Block carry no
    confidence. When confidence filtering drops words, the surviving Line/Block
    geometry is recomputed from the kept words so geometry and text agree.
    """
    height, width = page.dimensions

    blocks: list[Block] = []
    for db in page.blocks:
        lines: list[Line] = []
        for dl in db.lines:
            words = [
                Word(text=dw.value, confidence=float(dw.confidence), bbox=bbox, polygon=poly)
                for dw in dl.words
                if dw.confidence >= threshold
                for bbox, poly in [_geom_to_geometry(dw.geometry, width, height)]
            ]
            if not words:
                continue
            lines.append(
                Line(
                    text=" ".join(w.text for w in words),
                    bbox=_enclosing_bbox([w.bbox for w in words]),
                    words=words,
                )
            )
        if not lines:
            continue
        blocks.append(
            Block(
                text="\n".join(line.text for line in lines),
                kind=BlockKind.TEXT,
                bbox=_enclosing_bbox([line.bbox for line in lines]),
                lines=lines,
            )
        )

    return Page(page_number=1, width=float(width), height=float(height), blocks=blocks)


def _geom_to_geometry(geometry, width: int, height: int) -> tuple[BoundingBox, Polygon | None]:
    """Map docTR geometry (normalized) to a pixel bbox plus optional polygon.

    docTR returns either a 2-point box ``((xmin,ymin),(xmax,ymax))`` (straight
    pages) or a 4-point polygon (rotated pages, ``assume_straight_pages=False``).
    The bbox is always the axis-aligned extent; the polygon is populated only for
    the 4-point case.
    """
    pts = [(float(x) * width, float(y) * height) for x, y in geometry]
    xs = [x for x, _ in pts]
    ys = [y for _, y in pts]
    bbox = BoundingBox(x=min(xs), y=min(ys), width=max(xs) - min(xs), height=max(ys) - min(ys))
    polygon: Polygon | None = (pts[0], pts[1], pts[2], pts[3]) if len(pts) == 4 else None
    return bbox, polygon


def _enclosing_bbox(boxes: list[BoundingBox]) -> BoundingBox:
    """The axis-aligned box enclosing ``boxes`` (non-empty)."""
    x0 = min(b.x for b in boxes)
    y0 = min(b.y for b in boxes)
    x1 = max(b.x + b.width for b in boxes)
    y1 = max(b.y + b.height for b in boxes)
    return BoundingBox(x=x0, y=y0, width=x1 - x0, height=y1 - y0)
