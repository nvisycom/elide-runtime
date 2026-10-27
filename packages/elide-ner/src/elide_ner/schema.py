"""Schema inspection: what a request asks the model to do.

Pure functions over the request schema, kept out of the service because they
carry no service state and are what a reader checks when a batch is grouped
unexpectedly or a schema is refused.
"""

from __future__ import annotations

from elide_core.ner.v1 import NerRequest, Schema


def label_count(schema: Schema) -> int:
    """Every label the schema puts in front of the model.

    All three groups reach it (`build_schema` passes entities, classifications
    and structures alike), and each costs the same kind of work, so counting
    only `entities` bounds nothing: one entity plus two hundred classification
    tasks passes an entity-only check and still exhausts the service's memory —
    measured as an OOM-killed worker, which is the failure the limit exists to
    prevent.

    A classification contributes its own labels, not one per task, because that
    is what the model scores.
    """
    return (
        len(schema.entities)
        + sum(len(c.labels) for c in schema.classifications)
        + sum(len(st.fields) for st in schema.structures)
    )


def schema_key(req: NerRequest) -> tuple[float, str]:
    """A hashable key identifying requests that can share one batched call.

    Two requests batch together iff they have the same schema and threshold. The
    schema is serialized canonically (field order is stable) so equal schemas
    produce equal JSON, hence equal keys.
    """
    return (req.threshold, req.schema_.model_dump_json())
