"""Vision-language OCR service (PaddleOCR-VL) exposed over HTTP via BentoML.

Reads a document image and returns block-level regions (text + layout kind +
geometry + reading order) — the VLM's reading of the page. The runtime
reconciles this with a detection-OCR result; this service only reports what the
VLM read. Implements the ``nvisy_core.ocrvl.v1`` contract.

This is a GPU service (PaddleOCR-VL is a ~0.9B vision-language model); it runs
on CPU but slowly. Run locally::

    uv run bentoml serve nvisy_vl.service:OcrVlService --reload
"""

from __future__ import annotations

import base64
import binascii
import io
import os
from typing import TYPE_CHECKING

import bentoml
from bentoml.exceptions import BentoMLException, InternalServerError, InvalidArgument
from nvisy_core.ocrvl.v1 import Region, VlRequest, VlResponse
from nvisy_core.runtime import get_logger, request_id, resolve_model
from prometheus_client import Histogram

from nvisy_vl.block_map import block_kind

if TYPE_CHECKING:
    from PIL.Image import Image as PILImage

logger = get_logger("nvisy.vl")

# Built-in default model. Declared as the NVISY_MODEL_NAME env default below.
DEFAULT_MODEL = "PaddlePaddle/PaddleOCR-VL"

batch_size_metric = Histogram(
    "nvisy_ocrvl_batch_size",
    "Number of images merged into one recognize() call.",
    buckets=(1, 2, 4, 8),
)

# BentoML builds the image from this config; no hand-written Dockerfile. The
# requirements file is exported per-service from the workspace lock; bundled
# source is scoped by bentofile.yaml's `include`. PaddleOCR/OpenCV need libgl1.
# lock_python_packages=False: the file is already locked + hashed.
image = (
    bentoml.images.Image(python_version="3.12", lock_python_packages=False)
    .system_packages("libgl1", "libglib2.0-0")
    .requirements_file("packages/nvisy-vl/requirements.txt")
)


@bentoml.service(
    name="nvisy-inference-vl",
    image=image,
    # GPU service; a deployment without a GPU can set NVISY_DEVICE=cpu (slow).
    resources={"gpu": 1},
    traffic={"timeout": 120},
    envs=[
        {"name": "NVISY_MODEL_PATH", "value": "/models"},
        {"name": "NVISY_MODEL_NAME", "value": DEFAULT_MODEL},
        {"name": "NVISY_DEVICE", "value": "gpu"},
    ],
)
class OcrVlService:
    def __init__(self) -> None:
        from paddleocr import PaddleOCRVL

        self.model_id = resolve_model()
        device = os.getenv("NVISY_DEVICE", "gpu")
        logger.info("loading PaddleOCR-VL (model=%s, device=%s)", self.model_id, device)
        self.vl = PaddleOCRVL(device=device)
        logger.info("PaddleOCR-VL ready")

    # Sync (not async): inference is GPU-bound and blocking. BentoML runs sync
    # endpoints in a managed thread pool, so this never blocks the event loop.
    @bentoml.api(batchable=True, max_batch_size=4, max_latency_ms=200)
    def recognize(
        self,
        requests: list[VlRequest],
        ctx: bentoml.Context,
    ) -> list[VlResponse]:
        batch_size_metric.observe(len(requests))
        rid = request_id(ctx)
        logger.info("recognize batch=%d req_id=%s", len(requests), rid)
        try:
            return [self._recognize_one(req) for req in requests]
        except BentoMLException:
            raise
        except Exception as exc:
            logger.exception("inference failed (req_id=%s)", rid)
            raise InternalServerError("VL inference failed") from exc

    def _recognize_one(self, req: VlRequest) -> VlResponse:
        image = _decode_image(req.image)
        results = self.vl.predict(_to_ndarray(image))
        regions = _regions_from_result(results[0] if results else None)
        return VlResponse(regions=regions, model_id=self.model_id)


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


def _regions_from_result(result) -> list[Region]:
    """Map PaddleOCR-VL's ``parsing_res_list`` onto contract regions.

    ``result`` is a dict-like ``PaddleOCRVLResult``; ``parsing_res_list`` holds
    ``PaddleOCRVLBlock`` objects with attributes ``bbox`` ([x1,y1,x2,y2] px),
    ``label`` (layout label) and ``content`` (text). The blocks come in reading
    order, so the list index is the reading-order index.
    """
    from nvisy_core.ocr.v1 import BoundingBox

    if result is None:
        return []
    parsing = result.get("parsing_res_list") if hasattr(result, "get") else None
    if not parsing:
        return []

    regions: list[Region] = []
    for order, block in enumerate(parsing):
        x1, y1, x2, y2 = (float(v) for v in block.bbox)
        regions.append(
            Region(
                text=block.content or "",
                kind=block_kind(block.label or ""),
                bbox=BoundingBox(x=x1, y=y1, width=x2 - x1, height=y2 - y1),
                reading_order=order,
            )
        )
    return regions
