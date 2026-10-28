"""docTR model loading and wire-contract projection."""

from __future__ import annotations

import time
from typing import TYPE_CHECKING

from elide_core.ocr.v1 import Block, BlockKind, BoundingBox, Line, OcrResponse, Page, Polygon, Word
from elide_core.runtime import get_logger

if TYPE_CHECKING:
    from PIL.Image import Image as PILImage

from elide_ocr.runtime.config import OcrSettings

logger = get_logger("nvisy.ocr")


class Engine:
    def __init__(self, settings: OcrSettings | None = None) -> None:
        from doctr.models import ocr_predictor

        settings = settings if settings is not None else OcrSettings()
        self.model_id = settings.resolve_model()
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

    def recognize(self, image: PILImage, threshold: float) -> OcrResponse:
        result = self.ocr([_to_ndarray(image)])
        # We pass one image, so expect one page; guard an empty result instead
        # of indexing into nothing.
        pages = [_page_from_doctr(p, threshold) for p in getattr(result, "pages", [])]
        return OcrResponse(pages=pages, model_id=self.model_id)


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
