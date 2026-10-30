"""Engine construction: providers and device validation, with the predictor faked."""

from __future__ import annotations

import sys
import types

import pytest
from elide_ocr.inference.backends import onnxtr as backend
from elide_ocr.runtime.config import OcrSettings


@pytest.fixture
def built(monkeypatch):
    # Installed per test through sys.modules, which the engine's deferred
    # import reads, so the result does not depend on another module's fake.
    calls = []
    models = types.ModuleType("onnxtr.models")
    models.EngineConfig = lambda providers: providers
    models.ocr_predictor = lambda *args, **kwargs: calls.append((args, kwargs))
    models.from_hub = lambda repo, **kwargs: ("hub", repo, kwargs)
    monkeypatch.setitem(sys.modules, "onnxtr.models", models)
    return calls


def test_cpu_provider_is_always_named(built):
    """Unnamed, OnnxTR picks CoreML on macOS, which cannot build parseq."""
    engine = backend.Engine(OcrSettings(detection="fast_base", recognition="parseq"))
    assert engine.model_id == "fast_base+parseq"
    [(args, kwargs)] = built
    assert args == ("fast_base", "parseq")
    assert kwargs == {
        "det_engine_cfg": ["CPUExecutionProvider"],
        "reco_engine_cfg": ["CPUExecutionProvider"],
    }


def test_a_hub_model_loads_at_its_pinned_commit(built):
    sha = "7d7329948dbcff02637a3eb9c14ae0c4440c5ed0"
    backend.Engine(OcrSettings(recognition=f"Felix92/onnxtr-parseq-multilingual-v1@{sha}"))
    [(args, _)] = built
    assert args[0] == "db_resnet50"
    assert args[1] == (
        "hub",
        "Felix92/onnxtr-parseq-multilingual-v1",
        {"engine_cfg": ["CPUExecutionProvider"], "revision": sha},
    )


def test_cuda_without_the_gpu_runtime_fails_at_startup(built, monkeypatch):
    import onnxruntime

    monkeypatch.setattr(onnxruntime, "get_available_providers", lambda: ["CPUExecutionProvider"])
    with pytest.raises(RuntimeError, match="onnxruntime-gpu"):
        backend.Engine(OcrSettings(device="cuda"))
    assert built == []


def test_cuda_prefers_the_gpu_provider(built, monkeypatch):
    import onnxruntime

    available = ["CUDAExecutionProvider", "CPUExecutionProvider"]
    monkeypatch.setattr(onnxruntime, "get_available_providers", lambda: available)
    backend.Engine(OcrSettings(device="cuda"))
    [(_, kwargs)] = built
    assert kwargs["det_engine_cfg"] == ["CUDAExecutionProvider", "CPUExecutionProvider"]
