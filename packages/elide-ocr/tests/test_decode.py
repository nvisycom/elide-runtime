"""Invalid image payloads are distinguished from operational decode failures."""

import base64
import errno
import io

import pytest
from elide_ocr.inference.validation import InvalidInputError, _decode_image
from elide_ocr.runtime.metrics import rejected_metric
from PIL import Image


def image_bytes(format="PNG"):
    buffer = io.BytesIO()
    Image.new("RGB", (50, 50), "white").save(buffer, format=format)
    return buffer.getvalue()


def corrupt_png():
    data = bytearray(image_bytes())
    data[data.index(b"IDAT") + 4] = 0  # Damage the compressed stream's zlib header.
    return bytes(data)


@pytest.mark.parametrize(
    "data", [image_bytes("BMP")[:-50], corrupt_png()], ids=["truncated-bmp", "corrupt-png"]
)
def test_corrupt_image_is_rejected_and_counted(data):
    counter = rejected_metric.labels(reason="corrupt_image")
    before = counter._value.get()
    with pytest.raises(InvalidInputError, match="truncated or corrupt"):
        _decode_image(base64.b64encode(data).decode())
    assert counter._value.get() == before + 1


def test_decompression_bomb_is_rejected_and_counted(monkeypatch):
    data = image_bytes()
    monkeypatch.setattr(Image, "MAX_IMAGE_PIXELS", 100)
    counter = rejected_metric.labels(reason="decompression_bomb")
    before = counter._value.get()
    with pytest.raises(InvalidInputError, match="pixel limit"):
        _decode_image(base64.b64encode(data).decode())
    assert counter._value.get() == before + 1


@pytest.mark.parametrize(
    "error",
    [
        PermissionError(errno.EACCES, "permission denied"),
        OSError(errno.ENOMEM, "cannot load this image"),
        OSError("decoder jpeg not available"),
        OSError("out of memory error when reading image file"),
        MemoryError("allocation failed"),
    ],
)
def test_operational_failures_propagate(monkeypatch, error):
    data = image_bytes()

    def fail(*args, **kwargs):
        raise error

    monkeypatch.setattr(Image.Image, "convert", fail)
    counter = rejected_metric.labels(reason="corrupt_image")
    before = counter._value.get()
    with pytest.raises(type(error)) as excinfo:
        _decode_image(base64.b64encode(data).decode())
    assert excinfo.value is error
    assert counter._value.get() == before


def test_unidentified_image_keeps_existing_rejection_reason():
    counter = rejected_metric.labels(reason="not_an_image")
    before = counter._value.get()
    with pytest.raises(InvalidInputError, match="not a recognizable image"):
        _decode_image(base64.b64encode(b"not an image").decode())
    assert counter._value.get() == before + 1
