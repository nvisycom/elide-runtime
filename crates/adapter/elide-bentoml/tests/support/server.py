"""Run a real BentoML application with deterministic model libraries for Rust E2E tests.

Only model loading/execution is faked. Request parsing, dispatch, input decoding,
backend projection, and HTTP error handling are the production implementations.
"""

from __future__ import annotations

import os
import sys
import types
from types import SimpleNamespace


class Schema:
    def entities(self, labels):
        self.labels = labels
        return self


class Gliner:
    processor = SimpleNamespace(tokenizer=SimpleNamespace(encode=lambda text: text.split()))

    @classmethod
    def from_pretrained(cls, *args, **kwargs):
        return cls()

    def batch_extract(self, texts, schema, threshold=0.5, **kwargs):
        results = []
        for text in texts:
            if text == "model failure":
                raise RuntimeError("controlled model failure")
            start = text.find("Ada")
            span = {"text": "Ada", "confidence": 0.95, "start": start, "end": start + 3}
            results.append(
                {
                    "entities": {
                        label: [span] if start >= 0 and threshold <= 0.95 else []
                        for label in schema.labels
                    }
                }
            )
        return results


class Doctr:
    def __call__(self, images):
        height, width = images[0].shape[:2]
        words = [
            SimpleNamespace(value=value, confidence=confidence, geometry=geometry)
            for value, confidence, geometry in [
                ("Ada", 0.95, ((0.1, 0.2), (0.4, 0.4))),
                ("noise", 0.1, ((0.5, 0.2), (0.8, 0.4))),
            ]
        ]
        line = SimpleNamespace(words=words)
        page = SimpleNamespace(dimensions=(height, width), blocks=[SimpleNamespace(lines=[line])])
        return SimpleNamespace(pages=[page])


class Whisper:
    def __init__(self, *args, **kwargs):
        pass

    def transcribe(self, audio, language=None, word_timestamps=True):
        assert len(audio) == 16000, "the real decoder must resample 1s of 8 kHz audio to 16 kHz"
        word = SimpleNamespace(start=0.125, end=0.375, word="Ada", probability=0.9)
        segment = SimpleNamespace(
            start=0.125, end=0.375, text=" Ada ", words=[word] if word_timestamps else []
        )
        return iter([segment]), SimpleNamespace(language=language or "en")


def application(task):
    if task == "ner":
        os.environ.update(
            ELIDE_BENTO_NER_MODEL="e2e/ner",
            ELIDE_BENTO_NER_MAX_TOKENS="5",
            ELIDE_BENTO_NER_MAX_SCHEMA_BYTES="1024",
        )
        fake = types.ModuleType("gliner2")
        fake.GLiNER2 = Gliner
        fake.Schema = Schema
        sys.modules["gliner2"] = fake
        from elide_ner.service import NerService

        return NerService.to_asgi()
    if task == "ocr":
        os.environ["ELIDE_BENTO_MODEL_NAME"] = "e2e-det+e2e-rec"
        package = types.ModuleType("doctr")
        models = types.ModuleType("doctr.models")
        models.ocr_predictor = lambda *args, **kwargs: Doctr()
        package.models = models
        sys.modules.update({"doctr": package, "doctr.models": models})
        from elide_ocr.service import OcrService

        return OcrService.to_asgi()
    if task == "stt":
        os.environ.update(
            ELIDE_BENTO_MODEL_NAME="e2e/stt",
            ELIDE_BENTO_STT_DIARIZE="false",
            ELIDE_BENTO_STT_WORD_TIMESTAMPS="true",
            ELIDE_BENTO_STT_DEVICE="cpu",
            ELIDE_BENTO_STT_MAX_DURATION_SECONDS="1",
            ELIDE_BENTO_STT_MAX_BYTES="100000",
        )
        fake = types.ModuleType("faster_whisper")
        fake.WhisperModel = Whisper
        sys.modules["faster_whisper"] = fake
        from elide_stt.service import SttService

        return SttService.to_asgi()
    raise ValueError(f"unknown task: {task}")


def main():
    import uvicorn

    uvicorn.run(application(sys.argv[1]), host="0.0.0.0", port=3000, access_log=False)


if __name__ == "__main__":
    main()
