"""Shared base for the NER v1 wire models."""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict
from pydantic.alias_generators import to_camel


class _Model(BaseModel):
    """Base for every wire model: camelCase on the wire, both ways.

    Aliases apply on input (``populate_by_name`` also accepts the snake_case
    field name) AND on output (``serialize_by_alias``) so responses match the
    OpenAPI schema. ``protected_namespaces=()`` allows fields like ``model_id``.
    """

    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        serialize_by_alias=True,
        protected_namespaces=(),
    )
