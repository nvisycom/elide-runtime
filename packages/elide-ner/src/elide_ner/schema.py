"""Schema inspection: what a request asks the model to do.

Pure functions over the request schema, kept out of the service because they
carry no service state and are what a reader checks when a batch is grouped
unexpectedly or a schema is refused.
"""

from __future__ import annotations

from elide_core.ner.v1 import NerRequest, Schema


def schema_size(schema: Schema) -> int:
    """Bytes the schema serializes to — what the service bounds it by.

    Size rather than a count of labels, because counting bounds the wrong
    thing: every label, description and threshold reaches the model, and a
    count cannot see the text inside what it counts. Four attempts to bound
    this by counting were each bypassed by moving bytes somewhere the count
    did not look — a classification's labels, a structure field's `choices`,
    then a single entity's `description`, where 205 KB counted as one label
    and killed the worker.

    Serialized size has no such blind spot: anything a caller can put in the
    schema is in the JSON, so one number covers the labels, the descriptions
    and whatever a future field adds. It is also what `schema_key` already
    computes for batching, so the bound costs nothing a call did not pay.
    """
    return len(schema.model_dump_json().encode())


def schema_key(req: NerRequest) -> tuple[float, str]:
    """A hashable key identifying requests that can share one batched call.

    Two requests batch together iff they have the same schema and threshold. The
    schema is serialized canonically (field order is stable) so equal schemas
    produce equal JSON, hence equal keys.
    """
    return (req.threshold, req.schema_.model_dump_json())
