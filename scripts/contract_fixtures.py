"""Shared behavioral fixtures validated and serialized by Python wire models."""

from __future__ import annotations

import base64
import io
import wave

from elide_core.ner.v1 import Entity, EntitySpec, NerRequest, NerResponse, Schema, TokenUsage
from elide_core.ner.v1.contract import NerInput, NerOutput
from elide_core.ocr import v1 as ocr
from elide_core.stt import v1 as stt


def ner_fixtures() -> list[dict]:
    cases = []
    for text, entity in [
        ("Ada", "Ada"),
        ("Łódź Ada", "Ada"),
        ("🔒 李雷 e\u0301", "李雷"),
        ("e\u0301 Ada", "e\u0301"),
    ]:
        start = text.index(entity)
        end = start + len(entity)
        request = NerRequest(
            text=text,
            schema=Schema(
                entities=[
                    EntitySpec(
                        label="person", description="a person" if text.startswith("e") else None
                    )
                ]
            ),
        )
        response = NerResponse(
            model_id="contract/model",
            entities=[Entity(text=entity, label="person", score=0.9, start=start, end=end)],
            tokens=TokenUsage(input=3, limit=512),
        )
        cases.append(
            {
                "request": NerInput(requests=[request]).model_dump(mode="json"),
                "response": NerOutput([response]).model_dump(mode="json"),
                "expected_byte_range": [len(text[:start].encode()), len(text[:end].encode())],
            }
        )
    return cases


def ocr_fixtures() -> list[dict]:
    # A 100 x 50 RGB PNG, matching the response page dimensions.
    # Keep bytes fixed so export does not depend on Pillow or compression versions.
    image = (
        "iVBORw0KGgoAAAANSUhEUgAAAGQAAAAyCAIAAAAlV+npAAAAgklEQVR4nO3SsREAIAwDMWD/ncMK+V6q"
        "Xf35zsxh5y13iNV4ViBWIFYgViBWIFYgViBWIFYgViBWIFYgViBWIFYgViBWIFYgViBWIFYgViBWIFYg"
        "ViBWIFYgViBWIFYgViBWIFYgViBWIFYgViBWIFYgViBWIFYgViBWIFYgViDW2fto2wNhH84DkgAAAABJ"
        "RU5ErkJggg=="
    )
    bbox = ocr.BoundingBox(x=10, y=20, width=30, height=4)
    polygon = ((10, 20), (40, 20), (40, 24), (10, 24))
    word = ocr.Word(text="Hello", confidence=0.9, bbox=bbox, polygon=polygon)
    line = ocr.Line(text="Hello", bbox=bbox, words=[word], polygon=polygon)
    block = ocr.Block(text="Hello", kind=ocr.BlockKind.TABLE, bbox=bbox, lines=[line])
    cases = []
    for name, blocks, expected in [
        (
            "word-polygon",
            [block],
            [{"text": "Hello", "page": 2, "bbox": [10, 20, 30, 4], "confidence": 0.9}],
        ),
        (
            "line-fallback",
            [ocr.Block(text="line", bbox=bbox, lines=[ocr.Line(text="line", bbox=bbox)])],
            [{"text": "line", "page": 2, "bbox": [10, 20, 30, 4], "confidence": None}],
        ),
        (
            "block-fallback",
            [ocr.Block(text="block", bbox=bbox)],
            [{"text": "block", "page": 2, "bbox": [10, 20, 30, 4], "confidence": None}],
        ),
        ("empty", [], []),
    ]:
        response = ocr.OcrResponse(
            model_id="contract/ocr",
            pages=[ocr.Page(page_number=2, width=100, height=50, blocks=blocks)],
        )
        cases.append(
            {
                "name": name,
                "request": ocr.OcrInput(requests=[ocr.OcrRequest(image=image)]).model_dump(
                    mode="json"
                ),
                "response": ocr.OcrOutput([response]).model_dump(mode="json"),
                "expected_regions": expected,
            }
        )
    return cases


def stt_fixtures() -> list[dict]:
    buffer = io.BytesIO()
    with wave.open(buffer, "wb") as audio:
        audio.setnchannels(1)
        audio.setsampwidth(2)
        audio.setframerate(16000)
        audio.writeframes(b"\0\0" * 16)
    payload = base64.b64encode(buffer.getvalue()).decode()
    cases = []
    for name, segments, language in [
        (
            "timed-words",
            [
                stt.Segment(
                    start_ms=125,
                    end_ms=1375,
                    text="Hello world",
                    speaker_id="SPEAKER_00",
                    language="en",
                    confidence=0.9,
                    words=[
                        stt.Word(start_ms=125, end_ms=625, text="Hello", confidence=0.8),
                        stt.Word(start_ms=750, end_ms=1375, text="world"),
                    ],
                )
            ],
            "en",
        ),
        ("optional-fields", [stt.Segment(start_ms=0, end_ms=500, text="你好")], None),
        ("zero-duration", [stt.Segment(start_ms=500, end_ms=500, text="")], None),
        ("empty", [], None),
    ]:
        request = stt.SttInput(request=stt.SttRequest(audio=payload, language=language))
        response = stt.SttResponse(
            model_id="contract/stt", segments=segments, language=language, duration_ms=2000
        )
        cases.append(
            {
                "name": name,
                "request": request.model_dump(mode="json"),
                "response": stt.SttOutput(response).model_dump(mode="json"),
                "expected_segments": [segment.model_dump(mode="json") for segment in segments],
            }
        )
    return cases


def fixtures(service: str) -> list[dict]:
    return {"ner": ner_fixtures, "ocr": ocr_fixtures, "stt": stt_fixtures}[service]()
