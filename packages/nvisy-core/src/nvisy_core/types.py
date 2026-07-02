"""Shared scalar types used across the wire contracts."""

from __future__ import annotations

from typing import Annotated

from pydantic import Field

# A probability/score/confidence in the closed unit interval [0, 1].
Probability = Annotated[float, Field(ge=0.0, le=1.0)]
