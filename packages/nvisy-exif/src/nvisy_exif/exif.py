"""EXIF metadata reading and stripping for images.

Uses Pillow for EXIF handling. Supports JPEG, PNG, and TIFF formats.
These functions are designed to be callable from Rust via PyO3.
"""

from __future__ import annotations

import io

from PIL import Image
from PIL.ExifTags import TAGS


def read_exif(image_bytes: bytes) -> dict:
    """Read EXIF metadata from image bytes.

    Args:
        image_bytes: Raw image bytes (JPEG, PNG, or TIFF).

    Returns:
        Dictionary mapping human-readable tag names to their values.
        Binary or complex values are converted to strings.
    """
    img = Image.open(io.BytesIO(image_bytes))
    exif_data = img.getexif()

    result: dict[str, object] = {}
    for tag_id, value in exif_data.items():
        tag_name = TAGS.get(tag_id, str(tag_id))
        # Convert bytes to hex string for JSON compatibility
        if isinstance(value, bytes):
            value = value.hex()
        result[tag_name] = value

    return result


def strip_exif(image_bytes: bytes) -> bytes:
    """Remove all EXIF metadata from image bytes.

    Args:
        image_bytes: Raw image bytes (JPEG, PNG, or TIFF).

    Returns:
        Image bytes with all EXIF metadata removed, preserving the
        original format.
    """
    img = Image.open(io.BytesIO(image_bytes))
    fmt = img.format or "JPEG"

    # Create a clean copy without EXIF data
    clean = Image.new(img.mode, img.size)
    clean.putdata(list(img.getdata()))

    buf = io.BytesIO()
    clean.save(buf, format=fmt)
    return buf.getvalue()
