"""Tests for the container readiness probe.

Each branch is asserted because the probe's whole value is telling an
operator *which* failure they have, and because the timeout branch was
missing on the first attempt — the probe lived in a YAML string then, where
nothing could run it.
"""

from __future__ import annotations

import urllib.error
import urllib.request

import pytest
from elide_core import readyz


def _urlopen_raising(error: BaseException):
    def fake(*_args, **_kwargs):
        raise error

    return fake


def test_a_ready_service_exits_zero(monkeypatch):
    monkeypatch.setattr(urllib.request, "urlopen", lambda *a, **k: None)

    readyz.main()  # no SystemExit


def test_a_status_is_reported_as_loading(monkeypatch):
    """The server answered, so the process is alive but not serving yet."""
    error = urllib.error.HTTPError(readyz.URL, 503, "unavailable", {}, None)
    monkeypatch.setattr(urllib.request, "urlopen", _urlopen_raising(error))

    with pytest.raises(SystemExit) as excinfo:
        readyz.main()

    assert "loading" in str(excinfo.value)
    assert "503" in str(excinfo.value)


def test_a_read_timeout_is_reported_as_loading(monkeypatch):
    """The usual cold-start state: BentoML binds the port before the model
    finishes loading, so the connection is accepted and left unanswered.

    This must be caught before `URLError`, and must not escape as a
    traceback — which is what it did when the probe was a YAML string.
    """
    monkeypatch.setattr(urllib.request, "urlopen", _urlopen_raising(TimeoutError()))

    with pytest.raises(SystemExit) as excinfo:
        readyz.main()

    message = str(excinfo.value)
    assert "loading" in message, f"a timeout means loading, not down: {message}"
    assert "down" not in message


def test_a_refused_connection_is_reported_as_down(monkeypatch):
    """Nothing is listening: the process died or is crash-looping. The
    opposite response to `loading`, which is why they are distinguished."""
    error = urllib.error.URLError(ConnectionRefusedError(111, "Connection refused"))
    monkeypatch.setattr(urllib.request, "urlopen", _urlopen_raising(error))

    with pytest.raises(SystemExit) as excinfo:
        readyz.main()

    assert "down" in str(excinfo.value)


def test_the_probe_timeout_stays_under_the_healthcheck_timeout():
    """Compose allows the probe 5s. Answering just inside that is what keeps
    the reason visible: Docker records no output for a probe it killed."""
    assert readyz.TIMEOUT < 5.0
