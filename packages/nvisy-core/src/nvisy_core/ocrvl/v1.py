"""Vision-language OCR wire contract, version 1.

A VLM reads a document image and returns **block-level** regions — each with its
text, layout kind, bounding box, and reading-order index. This is a distinct,
leaner shape than the detection-OCR contract (``nvisy_core.ocr.v1``): the VLM
produces structured regions and high-accuracy text, not per-word geometry. The
runtime reconciles this with a detection-OCR result (geometry from OCR, text
refined by the VLM); the VL service itself only reports what the VLM read.

The Rust runtime (``nvisy-inference-client``) is the source of truth; these
models mirror it. The wire is camelCase to match the runtime's serde
``rename_all = "camelCase"``.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict, Field
from pydantic.alias_generators import to_camel

from nvisy_core.ocr.v1 import BlockKind, BoundingBox
from nvisy_core.types import Probability


class _Model(BaseModel):
    # camelCase on the wire, aliases on input and output; allow `model_id`.
    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        serialize_by_alias=True,
        protected_namespaces=(),
    )


class Region(_Model):
    """A layout region the VLM read: its text, kind, geometry, reading order."""

    text: str
    kind: BlockKind = Field(default=BlockKind.TEXT, description="Layout kind of the region.")
    bbox: BoundingBox
    reading_order: int = Field(ge=0, description="0-based reading-order index of the region.")
    confidence: Probability | None = None


class VlRequest(_Model):
    image: str = Field(description="Base64-encoded image bytes (PNG/JPEG/TIFF/WebP).")


class VlResponse(_Model):
    regions: list[Region] = Field(default_factory=list)
    model_id: str = Field(
        description="Identifier of the VLM that produced this result, for provenance.",
    )
