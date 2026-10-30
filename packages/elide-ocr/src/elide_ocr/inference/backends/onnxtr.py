"""OnnxTR model loading and wire-contract projection."""

from __future__ import annotations

import time
from typing import TYPE_CHECKING

from elide_core.ocr.v1 import Block, BlockKind, BoundingBox, Line, OcrResponse, Page, Polygon, Word
from elide_core.runtime import get_logger

if TYPE_CHECKING:
    from PIL.Image import Image as PILImage

from elide_ocr.runtime.config import DEVICE_ENV, HUB, OcrSettings

logger = get_logger("nvisy.ocr")

# Execution providers per device, in preference order.
PROVIDERS = {
    "cpu": ["CPUExecutionProvider"],
    "cuda": ["CUDAExecutionProvider", "CPUExecutionProvider"],
}


class Engine:
    def __init__(self, settings: OcrSettings | None = None) -> None:
        from onnxtr.models import EngineConfig, ocr_predictor

        settings = settings if settings is not None else OcrSettings()
        self.model_id = settings.model_id
        det, rec = settings.detection, settings.recognition
        if settings.device == "cuda":
            _require_cuda()
        logger.info("loading OnnxTR (det=%s, rec=%s, device=%s)", det, rec, settings.device)
        started = time.monotonic()
        try:
            # Providers always named: left to choose, OnnxTR prefers CoreML
            # wherever it is available, and CoreML cannot build parseq's graph
            # (ORT fails with "Unable to get shape for output:
            # /Squeeze_output_0"), so a macOS `make serve-ocr` would not run
            # what the images run.
            engine = EngineConfig(providers=PROVIDERS[settings.device])
            self.ocr = ocr_predictor(
                _model(det, engine),
                _model(rec, engine),
                det_engine_cfg=engine,
                reco_engine_cfg=engine,
            )
        except Exception as exc:
            # Log the cause here: the traceback a container log keeps is often
            # only its last lines, and the weights download during this call,
            # so the useful detail (an unknown architecture, a network or cache
            # failure) is where it is raised rather than where it surfaces.
            logger.exception("failed to load OnnxTR (det=%s, rec=%s): %s", det, rec, exc)
            raise RuntimeError(f"failed to load OnnxTR {self.model_id!r}: {exc}") from exc
        # Elapsed, because a cold start fetches weights and the difference
        # between "slow" and "wedged" is otherwise unknowable from the log.
        logger.info("OnnxTR ready in %.1fs", time.monotonic() - started)

    def recognize(self, image: PILImage, threshold: float) -> OcrResponse:
        result = self.ocr([_to_ndarray(image)])
        # We pass one image, so expect one page; guard an empty result instead
        # of indexing into nothing.
        pages = [_page_from_onnxtr(p, threshold) for p in getattr(result, "pages", [])]
        return OcrResponse(pages=pages, model_id=self.model_id)


def _model(ref: str, engine):
    """A built-in architecture name as is, or a Hub model loaded at its commit.

    ``ocr_predictor`` takes either: a name it resolves through OnnxTR's own
    cache, or a model already built. The Hub path goes through
    ``hf_hub_download``, so its weights sit in the Hugging Face cache, and the
    pinned revision is a commit, which resolves from that cache offline.
    """
    hub = HUB.fullmatch(ref)
    if hub is None:
        return ref
    from onnxtr.models import from_hub

    return from_hub(hub["repo"], engine_cfg=engine, revision=hub["revision"])


def _require_cuda() -> None:
    """Fail at startup rather than let ONNX Runtime fall back to CPU unasked.

    Given a provider it lacks, ORT logs a warning and runs on the next one, so
    a CPU-only image asked for "cuda" would serve, slowly, with nothing to say
    why but a log line.
    """
    import onnxruntime

    if "CUDAExecutionProvider" not in onnxruntime.get_available_providers():
        raise RuntimeError(
            f"{DEVICE_ENV}=cuda, but this ONNX Runtime has no CUDA provider; "
            "the image needs onnxruntime-gpu."
        )


def _to_ndarray(image: PILImage):
    import numpy as np

    return np.asarray(image)


def _page_from_onnxtr(page, threshold: float) -> Page:
    """Map an OnnxTR page onto the contract.

    OnnxTR keeps docTR's document model. Geometry is normalized [0, 1] relative
    to the page; ``dimensions`` is ``(height, width)`` in pixels. We scale
    geometry to pixels and keep the full Block -> Line -> Word hierarchy. The
    predictor runs no layout classification, so blocks default to
    BlockKind.TEXT.

    Confidence is only available at word level, so Line/Block carry no
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
    """Map OnnxTR geometry (normalized) to a pixel bbox plus optional polygon.

    OnnxTR returns either a 2-point box ``((xmin,ymin),(xmax,ymax))`` (straight
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
