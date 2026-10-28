"""HTTP error mapping and transcription with a fake backend."""

import base64
import io
import wave

import pytest
from elide_core.stt.v1 import SttResponse
from elide_stt.inference import backends
from elide_stt.inference.validation import SAMPLE_RATE, AudioTooLongError
from elide_stt.runtime import config


@pytest.fixture
def client(monkeypatch):
    from prometheus_client import REGISTRY

    # BentoML registers process-global HTTP metrics for each test server.
    for collector in list(REGISTRY._collector_to_names):
        REGISTRY.unregister(collector)

    from elide_stt.service import SttService
    from starlette.testclient import TestClient

    settings = config.SttSettings(max_duration_seconds=1, max_bytes=100_000)
    monkeypatch.setattr(config, "get_settings", lambda: settings)

    class Backend:
        model_id = "fake"
        device = "cpu"
        diarizes = False
        max_duration_seconds = 1

        def check_duration(self, audio):
            seconds = len(audio) / SAMPLE_RATE
            if seconds > 1:
                raise AudioTooLongError("audio exceeds one second")
            return seconds

        def transcribe(self, audio, language=None):
            return SttResponse(
                model_id="fake",
                segments=[],
                language=language,
                duration_ms=round(len(audio) / SAMPLE_RATE * 1000),
            )

    monkeypatch.setattr(backends, "create", lambda received: Backend())
    with TestClient(SttService.to_asgi()) as client:
        yield client


def wav_payload(seconds):
    output = io.BytesIO()
    with wave.open(output, "wb") as audio:
        audio.setnchannels(1)
        audio.setsampwidth(2)
        audio.setframerate(SAMPLE_RATE)
        audio.writeframes(b"\0\0" * round(seconds * SAMPLE_RATE))
    return base64.b64encode(output.getvalue()).decode()


def test_transcribes_valid_audio(client):
    response = client.post(
        "/transcribe", json={"request": {"audio": wav_payload(0.5), "language": "en"}}
    )
    assert response.status_code == 200
    assert response.json()["modelId"] == "fake"
    assert response.json()["durationMs"] == 500
    assert response.json()["language"] == "en"


@pytest.mark.parametrize(
    "audio", ["bad base64!", "", wav_payload(1.5)], ids=["malformed", "empty", "overlong"]
)
def test_invalid_audio_returns_400(client, audio):
    response = client.post("/transcribe", json={"request": {"audio": audio}})
    assert response.status_code == 400
