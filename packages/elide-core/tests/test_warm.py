"""Tests for the shared cache-warmer helpers.

Each service has its own ``warm`` module; what is tested here is the part they
share, and in particular the ownership handover, which is where this went
wrong once already.
"""

from __future__ import annotations

import os

import pytest
from elide_core import warm


def test_a_missing_variable_exits_naming_it(monkeypatch):
    """A deployment error gets a message, not a traceback."""
    monkeypatch.delenv("SOME_MODEL_ID", raising=False)

    with pytest.raises(SystemExit) as excinfo:
        warm.require_env("SOME_MODEL_ID")

    assert "SOME_MODEL_ID" in str(excinfo.value)


def test_an_empty_variable_is_treated_as_missing(monkeypatch):
    """Compose writes ``KEY: ${VAR:-}`` for an unset optional, so an empty
    string reaches a warmer as readily as an absent variable."""
    monkeypatch.setenv("SOME_MODEL_ID", "")

    with pytest.raises(SystemExit) as excinfo:
        warm.require_env("SOME_MODEL_ID")

    assert "SOME_MODEL_ID" in str(excinfo.value)


def test_a_set_variable_is_returned(monkeypatch):
    monkeypatch.setenv("SOME_MODEL_ID", "org/model")

    assert warm.require_env("SOME_MODEL_ID") == "org/model"


def test_the_cache_is_handed_to_the_service_uid(tmp_path, monkeypatch):
    """Root writes the cache; the service reads it as another uid.

    Without the handover the service fails at startup with a PermissionError
    on a directory it is supposed to own. ``os.lchown`` is patched because the
    test does not run as root, so the assertion is on *what* would change —
    every path under the cache, not only its root.
    """
    nested = tmp_path / "hub" / "models--org--m"
    nested.mkdir(parents=True)
    (nested / "weights.bin").write_text("x")

    chowned: list[tuple[str, int, int]] = []
    monkeypatch.setattr(os, "lchown", lambda path, uid, gid: chowned.append((str(path), uid, gid)))
    # Guard against regressing to `chown`: it follows symlinks, so a symlinked
    # weight file has its blob changed rather than the link, and whichever
    # rglob reached first decided the outcome. Measured at 12 of 61 paths left
    # root-owned, which is enough for the service to fail reading its cache.
    monkeypatch.setattr(os, "chown", lambda *a, **k: pytest.fail("use lchown"))

    warm.hand_over(str(tmp_path))

    paths = {path for path, _, _ in chowned}
    assert str(tmp_path) in paths, "the cache root itself must be handed over"
    assert str(nested / "weights.bin") in paths, (
        "nested files must be handed over, not only the top level"
    )
    assert all((uid, gid) == (warm.SERVICE_UID, warm.SERVICE_GID) for _, uid, gid in chowned)


def test_a_symlinked_weight_file_has_the_link_changed(tmp_path, monkeypatch):
    """The Hugging Face layout in miniature: a blob, and a snapshot symlink
    pointing at it. Both must be handed over, and via the link rather than
    through it."""
    blobs = tmp_path / "hub" / "blobs"
    snaps = tmp_path / "hub" / "snapshots" / "abc"
    blobs.mkdir(parents=True)
    snaps.mkdir(parents=True)
    blob = blobs / "deadbeef"
    blob.write_text("weights")
    (snaps / "model.safetensors").symlink_to(blob)

    seen: list[str] = []
    monkeypatch.setattr(os, "lchown", lambda path, uid, gid: seen.append(str(path)))
    monkeypatch.setattr(os, "chown", lambda *a, **k: pytest.fail("use lchown"))

    warm.hand_over(str(tmp_path))

    assert str(blob) in seen, "the blob holding the weights must be handed over"
    assert str(snaps / "model.safetensors") in seen, (
        "the symlink itself must be handed over, not its target"
    )
