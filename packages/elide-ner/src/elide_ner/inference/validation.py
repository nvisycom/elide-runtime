"""Schema inspection: what a request asks the model to do.

Pure functions over the request schema, kept out of the service because they
carry no service state and are what a reader checks when a batch is grouped
unexpectedly or a schema is refused.
"""

from __future__ import annotations

from elide_core.ner.v1 import Schema


def schema_size(schema: Schema) -> int:
    """Measure the serialized schema, including labels and descriptions."""
    return len(schema.model_dump_json().encode())


class TextTooLongError(ValueError):
    """Input exceeds the model's token budget."""


class SchemaTooLargeError(ValueError):
    """Schema exceeds the configured byte budget."""


def check_schema_size(size: int, limit: int) -> None:
    """Apply the same schema budget at parsing and backend boundaries."""
    if size > limit:
        raise SchemaTooLargeError(f"schema is {size} bytes; the limit is {limit}")
