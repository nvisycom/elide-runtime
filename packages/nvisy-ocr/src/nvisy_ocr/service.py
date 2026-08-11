"""OCR inference service (docTR) exposed over HTTP via BentoML.

The default implementation of the OCR wire contract (``nvisy_core.ocr.v1``).
docTR natively produces a ``Document -> Page -> Block -> Line -> Word`` hierarchy
with word-level geometry, which maps directly onto our contract.

Run locally::

    uv run bentoml serve nvisy_ocr.service:OcrService --reload
"""

from __future__ import annotations

import base64
import binascii
import io
from typing import TYPE_CHECKING

import bentoml
from bentoml.exceptions import BentoMLException, InternalServerError, InvalidArgument
from nvisy_core.ocr.v1 import (
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
from nvisy_core.runtime import get_logger, request_id, resolve_model
from prometheus_client import Histogram

if TYPE_CHECKING:
    from PIL.Image import Image as PILImage

logger = get_logger("nvisy.ocr")

# Built-in default detection + recognition models, joined as "det+rec".
# Declared as the NVISY_MODEL_NAME env default below (single source of truth).
DEFAULT_MODEL = "db_resnet50+crnn_vgg16_bn"

batch_size_metric = Histogram(
    "nvisy_ocr_batch_size",
    "Number of images merged into one recognize() call.",
    buckets=(1, 2, 4, 8, 16),
)

# BentoML builds the image from this config (`bentoml build` + `containerize`);
# no hand-written Dockerfile. The requirements file is exported per-service from
# the workspace lock (scripts/gen_requirements.py); bundled source is scoped by
# bentofile.yaml's `include`. docTR/OpenCV need libgl1. lock_python_packages=
# False: the file is already locked + hashed, so BentoML must not re-resolve it.
image = (
    bentoml.images.Image(python_version="3.12", lock_python_packages=False)
    .system_packages("libgl1", "libglib2.0-0")
    .requirements_file("packages/nvisy-ocr/requirements.txt")
)


@bentoml.service(
    name="nvisy-inference-ocr",
    image=image,
    resources={"cpu": "2"},
    traffic={"timeout": 60},
    # Declared with defaults so they're optional + documented in the bento
    # manifest. NVISY_MODEL_PATH defaults to the /models mount (empty unless BYO
    # weights are mounted); NVISY_MODEL_NAME is the "det+rec" pair otherwise.
    envs=[
        {"name": "NVISY_MODEL_PATH", "value": "/models"},
        {"name": "NVISY_MODEL_NAME", "value": DEFAULT_MODEL},
    ],
)
class OcrService:
    def __init__(self) -> None:
        from doctr.models import ocr_predictor

        self.model_id = resolve_model()
        det, _, rec = self.model_id.partition("+")
        if not rec:
            raise RuntimeError(f"NVISY_MODEL_NAME must be 'det+rec', got {self.model_id!r}")
        logger.info("loading docTR (det=%s, rec=%s)", det, rec)
        self.ocr = ocr_predictor(det, rec, pretrained=True)
        logger.info("docTR ready")

    # Sync (not async): inference is CPU/GPU-bound and blocking. BentoML runs
    # sync endpoints in a managed thread pool, so this never blocks the event
    # loop (an async def here would, and could starve /readyz).
    @bentoml.api(batchable=True, max_batch_size=8, max_latency_ms=120)
    def recognize(
        self,
        requests: list[OcrRequest],
        ctx: bentoml.Context,
    ) -> list[OcrResponse]:
        batch_size_metric.observe(len(requests))
        rid = request_id(ctx)
        logger.info("recognize batch=%d req_id=%s", len(requests), rid)
        try:
            return [self._recognize_one(req) for req in requests]
        except BentoMLException:
            # Typed request errors (e.g. InvalidArgument for bad image bytes)
            # carry their own status; let them through unchanged.
            raise
        except Exception as exc:
            logger.exception("inference failed (req_id=%s)", rid)
            raise InternalServerError("OCR inference failed") from exc

    def _recognize_one(self, req: OcrRequest) -> OcrResponse:
        image = _decode_image(req.image)
        result = self.ocr([_to_ndarray(image)])
        # We pass one image, so expect one page; guard an empty result instead
        # of indexing into nothing.
        pages = [
            _page_from_doctr(p, req.confidence_threshold) for p in getattr(result, "pages", [])
        ]
        return OcrResponse(pages=pages, model_id=self.model_id)


def _decode_image(image_b64: str) -> PILImage:
    from PIL import Image, UnidentifiedImageError

    try:
        raw = base64.b64decode(image_b64, validate=True)
    except (binascii.Error, ValueError) as exc:
        raise InvalidArgument("image is not valid base64") from exc
    try:
        return Image.open(io.BytesIO(raw)).convert("RGB")
    except UnidentifiedImageError as exc:
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
