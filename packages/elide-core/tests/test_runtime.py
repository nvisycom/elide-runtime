"""Tests for the shared runtime helpers."""

from elide_core.runtime import request_id


def test_request_id_none_without_headers():
    assert request_id(object()) is None


def test_request_id_reads_header():
    class _Ctx:
        request = type("R", (), {"headers": {"x-request-id": "abc-123"}})()

    assert request_id(_Ctx()) == "abc-123"
