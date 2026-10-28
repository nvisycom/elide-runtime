"""Schemas and shared fixtures are owned by the Python models for every service."""

import base64
import io
import json
import re
import wave
from pathlib import Path

import pytest
from elide_core.ner.v1.contract import NerInput, NerOutput
from elide_core.ocr.v1 import OcrInput, OcrOutput
from elide_core.stt.v1 import SttInput, SttOutput

CONTRACTS = Path(__file__).resolve().parents[3] / "contracts"
MODELS = {"ner": (NerInput, NerOutput), "ocr": (OcrInput, OcrOutput), "stt": (SttInput, SttOutput)}


@pytest.mark.parametrize("service", MODELS)
@pytest.mark.parametrize(
    "side, mode, index", [("request", "validation", 0), ("response", "serialization", 1)]
)
def test_exported_schema_matches_python_and_uses_snake_case(service, side, mode, index):
    schema = MODELS[service][index].model_json_schema(by_alias=True, mode=mode)
    schema["$schema"] = "https://json-schema.org/draft/2020-12/schema"
    assert json.loads((CONTRACTS / service / "v1" / f"{side}.schema.json").read_text()) == schema

    def check_field_names(value):
        if isinstance(value, dict):
            for field in value.get("properties", {}):
                assert re.fullmatch(r"[a-z][a-z0-9_]*", field), field
            for child in value.values():
                check_field_names(child)
        elif isinstance(value, list):
            for child in value:
                check_field_names(child)

    check_field_names(schema)


@pytest.mark.parametrize("service", MODELS)
def test_shared_fixtures_validate(service):
    request_model, response_model = MODELS[service]
    cases = json.loads((CONTRACTS / service / "v1/fixtures.json").read_text())
    for case in cases:
        request_model.model_validate(case["request"])
        response_model.model_validate(case["response"])


def test_ocr_fixtures_preserve_rotated_geometry_and_fallback_content():
    cases = json.loads((CONTRACTS / "ocr/v1/fixtures.json").read_text())
    for case in cases:
        request = OcrInput.model_validate(case["request"])
        assert base64.b64decode(request.requests[0].image, validate=True).startswith(b"\x89PNG")
        response = OcrOutput.model_validate(case["response"])
        actual = []
        for page in response.root[0].pages:
            for block in page.blocks:
                items = [word for line in block.lines for word in (line.words or [line])]
                for item in items or [block]:
                    box = item.bbox
                    actual.append(
                        {
                            "text": item.text,
                            "page": page.page_number,
                            "bbox": [box.x, box.y, box.width, box.height],
                            "confidence": getattr(item, "confidence", None),
                        }
                    )
        assert actual == case["expected_regions"]
    rotated = OcrOutput.model_validate(cases[0]["response"]).root[0].pages[0].blocks[0]
    assert rotated.lines[0].words[0].polygon[2] == (40, 24)


def test_stt_fixtures_use_integer_milliseconds_and_valid_audio():
    cases = json.loads((CONTRACTS / "stt/v1/fixtures.json").read_text())
    for case in cases:
        request = SttInput.model_validate(case["request"])
        with wave.open(io.BytesIO(base64.b64decode(request.request.audio)), "rb") as audio:
            assert audio.getframerate() == 16000
        response = SttOutput.model_validate(case["response"])
        assert [segment.model_dump(mode="json") for segment in response.root.segments] == case[
            "expected_segments"
        ]
