"""Shared base for the NER v1 wire models."""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict


class _Model(BaseModel):
    """Snake_case wire models, preserving the reserved ``schema_`` → ``schema`` alias."""

    model_config = ConfigDict(
        populate_by_name=True,
        serialize_by_alias=True,
        protected_namespaces=(),
    )
