"""Decode incoming image payloads before inference."""

from __future__ import annotations

import base64
import binascii
import io
from typing import TYPE_CHECKING

from elide_core.runtime import get_logger

from elide_ocr.runtime.metrics import rejected_metric

if TYPE_CHECKING:
    from PIL.Image import Image as PILImage

logger = get_logger("nvisy.ocr")

# Pillow uses plain OSError for both invalid data and operational failures.
# Only known input-error messages with no system errno are caller rejections.
_INVALID_IMAGE_ERRORS = (
    "truncated",
    "broken data stream",
    "unrecognized data stream",
    "decoding error",
    "image buffer overrun",
    "cannot load this image",
    "corrupt",
)


class InvalidInputError(ValueError):
    """Payload cannot be decoded or exceeds an input budget."""


def _decode_image(image_b64: str, rid: str | None = None) -> PILImage:
    from PIL import Image, UnidentifiedImageError

    try:
        raw = base64.b64decode(image_b64, validate=True)
    except (binascii.Error, ValueError) as exc:
        rejected_metric.labels(reason="base64").inc()
        logger.warning("rejected: image is not valid base64 (req_id=%s)", rid)
        raise InvalidInputError("image is not valid base64") from exc
    try:
        with Image.open(io.BytesIO(raw)) as image:
            return image.convert("RGB")
    except UnidentifiedImageError as exc:
        rejected_metric.labels(reason="not_an_image").inc()
        logger.warning("rejected: image bytes are not a recognizable image (req_id=%s)", rid)
        raise InvalidInputError("image bytes are not a recognizable image") from exc
    except Image.DecompressionBombError as exc:
        rejected_metric.labels(reason="decompression_bomb").inc()
        logger.warning("rejected: image exceeds the pixel limit (req_id=%s)", rid)
        raise InvalidInputError("image exceeds the pixel limit") from exc
    except OSError as exc:
        if exc.errno is not None or not any(
            message in str(exc).lower() for message in _INVALID_IMAGE_ERRORS
        ):
            raise
        rejected_metric.labels(reason="corrupt_image").inc()
        logger.warning("rejected: image data is truncated or corrupt (req_id=%s)", rid)
        raise InvalidInputError("image data is truncated or corrupt") from exc
