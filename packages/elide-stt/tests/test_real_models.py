"""Real-model tests: exercise the pipeline against real Whisper weights.

Marked ``real`` and excluded from the default suite (which fakes the model).
Run via ``pytest -m real`` (the ``real-models`` CI job). These catch what fakes
cannot: a hint faster-whisper refuses, a decoder that cannot open real audio,
or timings that do not fit the clip.

They load ``tiny`` rather than the 1.6 GB default: the code under test is the
same for every faster-whisper checkpoint.
"""

from __future__ import annotations

import base64
import os
import subprocess
import sys

import pytest

pytestmark = pytest.mark.real

MODEL = "tiny"

# One LibriSpeech utterance (CC-BY-4.0), about 10.4 s, at a pinned commit.
SAMPLE_REPO = "Narsil/asr_dummy"
SAMPLE_FILE = "1.flac"
SAMPLE_REVISION = "8d141c84e3f84c54cd7bbaa851d24edd0f559734"


@pytest.fixture(scope="module")
def audio() -> str:
    from huggingface_hub import hf_hub_download

    path = hf_hub_download(SAMPLE_REPO, SAMPLE_FILE, repo_type="dataset", revision=SAMPLE_REVISION)
    with open(path, "rb") as f:
        return base64.b64encode(f.read()).decode()


@pytest.fixture(scope="module")
def pipeline():
    from elide_stt.inference.backends.faster_whisper import Engine
    from elide_stt.inference.pipeline import SttPipeline
    from elide_stt.runtime.config import SttSettings

    settings = SttSettings(model_id=MODEL)
    return SttPipeline(Engine(settings), settings)


def test_transcribes_with_a_regional_hint(pipeline, audio):
    from elide_core.stt.v1 import SttRequest

    # The runtime sends the call's primary language as BCP-47, which
    # faster-whisper refuses as is.
    resp = pipeline.transcribe(SttRequest(audio=audio, language="en-US"))

    assert resp.language == "en"
    text = " ".join(s.text for s in resp.segments).lower()
    assert "stew for dinner" in text
    for segment in resp.segments:
        assert 0 <= segment.start_ms < segment.end_ms <= resp.duration_ms
        assert segment.words, "word timestamps are on by default"
        for word in segment.words:
            assert segment.start_ms <= word.start_ms <= word.end_ms <= segment.end_ms


def test_offline_load(pipeline):
    # `pipeline` fills the cache this loads from. In a fresh process:
    # huggingface_hub reads HF_HUB_OFFLINE when it is first imported, so
    # setting it in this one would not take effect. The service runs offline,
    # and the model must load from that cache.
    script = (
        "from elide_stt.inference.backends.faster_whisper import Engine\n"
        "from elide_stt.runtime.config import SttSettings\n"
        f"Engine(SttSettings(model_id={MODEL!r}))\n"
    )
    env = {**os.environ, "HF_HUB_OFFLINE": "1"}
    result = subprocess.run([sys.executable, "-c", script], env=env, capture_output=True, text=True)
    assert result.returncode == 0, result.stderr[-2000:]
