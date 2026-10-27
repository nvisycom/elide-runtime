"""Schema inspection: what a request asks the model to do.

Pure functions over the request schema, kept out of the service because they
carry no service state and are what a reader checks when a batch is grouped
unexpectedly or a schema is refused.
"""

from __future__ import annotations

from elide_core.ner.v1 import NerRequest, Schema


def label_count(schema: Schema) -> int:
    """Every label the schema puts in front of the model.

    Entities are the only group the schema carries: classifications and
    structures were removed because their contents reach the model and so
    were a way past this very bound — a field's `choices` are embedded, so one
    field with five thousand of them counted as a single label and killed the
    worker.
    """
    return len(schema.entities)


def schema_key(req: NerRequest) -> tuple[float, str]:
    """A hashable key identifying requests that can share one batched call.

    Two requests batch together iff they have the same schema and threshold. The
    schema is serialized canonically (field order is stable) so equal schemas
    produce equal JSON, hence equal keys.
    """
    return (req.threshold, req.schema_.model_dump_json())
