"""Real-model tests: exercise the engine against the default OnnxTR weights.

Marked ``real`` and excluded from the default suite (which fakes the
predictor). Run via ``pytest -m real`` (the ``real-models`` CI job). These
catch what fakes cannot: the pinned Hub recognizer no longer resolving, a
change to OnnxTR's loading or output, or a projection that puts words in the
wrong place on the page.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import pytest

pytestmark = pytest.mark.real

# Names the default recognizer exists to read: none is in the French
# vocabulary of OnnxTR's built-in models.
WORDS = ["Łukasz", "Dvořák", "Schäfer", "García"]

# A font with those letters: DejaVu on Linux (CI installs fonts-dejavu-core),
# Arial on macOS.
FONTS = [
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/System/Library/Fonts/Supplemental/Arial.ttf",
]


def _page():
    """A line of `WORDS` on a white page, with each word's drawn box."""
    from PIL import Image, ImageDraw, ImageFont

    font_path = next((f for f in FONTS if Path(f).exists()), None)
    if font_path is None:
        pytest.fail(f"no font with Polish and Czech letters; looked for {FONTS}")
    font = ImageFont.truetype(font_path, 48)
    image = Image.new("RGB", (1200, 300), "white")
    draw = ImageDraw.Draw(image)
    boxes, x = {}, 80
    for word in WORDS:
        boxes[word] = draw.textbbox((x, 120), word, font=font)
        draw.text((x, 120), word, fill="black", font=font)
        x = boxes[word][2] + 40
    return image, boxes


def _iou(a, b) -> float:
    ix = max(0, min(a[2], b[2]) - max(a[0], b[0]))
    iy = max(0, min(a[3], b[3]) - max(a[1], b[1]))
    inter = ix * iy
    return inter / ((a[2] - a[0]) * (a[3] - a[1]) + (b[2] - b[0]) * (b[3] - b[1]) - inter)


@pytest.fixture(scope="module")
def engine():
    from elide_ocr.inference.backends.onnxtr import Engine

    return Engine()


def test_reads_names_where_they_are_printed(engine):
    image, drawn = _page()
    [page] = engine.recognize(image, threshold=0.0).pages

    words = {w.text: w for b in page.blocks for line in b.lines for w in line.words}
    assert set(WORDS) <= set(words), f"read {sorted(words)}"
    for text, box in drawn.items():
        b = words[text].bbox
        assert _iou((b.x, b.y, b.x + b.width, b.y + b.height), box) > 0.5, text


def test_offline_load_and_infer(engine):
    # `engine` fills the cache this loads from. In a fresh process:
    # huggingface_hub reads HF_HUB_OFFLINE when it is first imported, so
    # setting it in this one would not take effect. The service runs offline,
    # and the pinned recognizer must resolve from that cache.
    script = (
        "from PIL import Image\n"
        "from elide_ocr.inference.backends.onnxtr import Engine\n"
        "Engine().recognize(Image.new('RGB', (200, 100), 'white'), threshold=0.0)\n"
    )
    env = {**os.environ, "HF_HUB_OFFLINE": "1"}
    result = subprocess.run([sys.executable, "-c", script], env=env, capture_output=True, text=True)
    assert result.returncode == 0, result.stderr[-2000:]
