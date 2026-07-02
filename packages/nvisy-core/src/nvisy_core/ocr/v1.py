"""OCR wire contract, version 1.

Request carries a base64-encoded image; response is a ``Page -> Block -> Line ->
Word`` hierarchy. Geometry is an axis-aligned bounding box (pixels, origin
top-left) plus an optional 4-point polygon for engines that report rotated
regions. Blocks carry a :class:`BlockKind` from layout analysis.

The Rust runtime (``nvisy-inference-client``) is the source of truth; these
models mirror it. The wire is camelCase to match the runtime's serde
``rename_all = "camelCase"``.
"""

from __future__ import annotations

from enum import StrEnum

from pydantic import BaseModel, ConfigDict, Field
from pydantic.alias_generators import to_camel

from nvisy_core.types import Probability

# [x, y] image-space point. Origin top-left, pixels.
Point = tuple[float, float]
# A 4-point polygon: [top-left, top-right, bottom-right, bottom-left].
Polygon = tuple[Point, Point, Point, Point]


class BlockKind(StrEnum):
    """Layout kind of a block. Mirrors the runtime's ``BlockKind``."""

    TEXT = "text"
    TABLE = "table"
    FIGURE = "figure"
    OTHER = "other"


class _Model(BaseModel):
    # camelCase on the wire: aliases on input (populate_by_name accepts either)
    # AND on output (serialize_by_alias) so responses match the OpenAPI schema.
    # protected_namespaces=(): allow fields like `model_id`.
    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        serialize_by_alias=True,
        protected_namespaces=(),
    )


class BoundingBox(_Model):
    """Axis-aligned box in image pixels, origin top-left."""

    x: float
    y: float
    width: float = Field(ge=0.0)
    height: float = Field(ge=0.0)


class Word(_Model):
    text: str
    confidence: Probability | None = None
    bbox: BoundingBox
    polygon: Polygon | None = Field(
        default=None,
        description="4-point polygon, present when the engine reports rotated regions.",
    )


class Line(_Model):
    text: str
    # Confidence is reported at word level; engines that don't aggregate to the
    # line (e.g. docTR) leave this null.
    confidence: Probability | None = None
    bbox: BoundingBox
    polygon: Polygon | None = None
    words: list[Word] = Field(default_factory=list)


class Block(_Model):
    text: str
    kind: BlockKind = Field(default=BlockKind.TEXT, description="Layout kind of the block.")
    bbox: BoundingBox
    lines: list[Line] = Field(default_factory=list)


class Page(_Model):
    page_number: int = Field(ge=1, description="1-based page index.")
    width: float | None = Field(default=None, description="Page width in pixels.")
    height: float | None = Field(default=None, description="Page height in pixels.")
    blocks: list[Block] = Field(default_factory=list)


class OcrRequest(_Model):
    image: str = Field(description="Base64-encoded image bytes (PNG/JPEG/TIFF/WebP).")
    confidence_threshold: Probability = Field(
        default=0.0,
        description="Drop words below this recognition confidence.",
    )


class OcrResponse(_Model):
    pages: list[Page] = Field(default_factory=list)
    model_id: str = Field(
        description="Identifier of the model that produced this result, for "
        "provenance (e.g. the engine's model name/pair). Does not encode the "
        "engine library version.",
    )
