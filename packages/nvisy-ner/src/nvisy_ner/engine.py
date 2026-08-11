"""GLiNER2 engine: load the model, translate the wire schema, run extraction.

The heavy ``gliner2`` import lives inside :func:`_load` so importing this module
stays cheap.

Translation happens at two boundaries:

- **schema in:** the wire :class:`~nvisy_core.ner.v1.Schema` →
  ``gliner2.Schema`` (entities / classifications / structures, with per-field
  regex validators).
- **result out:** gliner2's ``batch_extract`` result dict → the typed
  :class:`~nvisy_core.ner.v1.NerResponse` groups (gliner2's ``confidence`` is
  exposed as ``score``).
"""

from __future__ import annotations

from typing import TYPE_CHECKING, TypedDict, cast

from nvisy_core.ner.v1 import (
    Classification,
    ClassLabel,
    Entity,
    EntitySpec,
    NerResponse,
    Record,
    Schema,
    Span,
)

from nvisy_ner import config

if TYPE_CHECKING:
    from gliner2 import GLiNER2, RegexValidator
    from gliner2 import Schema as G2Schema


# ---- gliner2 batch_extract result shapes (verified against the real model) ----
# include_spans=True, include_confidence=True. A span dict carries character
# offsets + confidence; a class result is one label dict or a list of them; a
# structure is a list of records (field name -> spans).


class _G2Span(TypedDict):
    text: str
    confidence: float
    start: int
    end: int


class _G2ClassLabel(TypedDict):
    label: str
    confidence: float


# entities: {label: [span, ...]}; classification: label-dict or list; structure:
# [ {field: [span, ...]}, ... ]. The result dict mixes these, keyed by task name.
_G2Group = (
    dict[str, list[_G2Span]]  # entities
    | _G2ClassLabel  # single-label classification
    | list[_G2ClassLabel]  # multi-label classification
    | list[dict[str, list[_G2Span]]]  # structures
)
_G2Result = dict[str, _G2Group]

# A gliner2 entity-dict value: a description string (empty = bare label), or
# the {description?, threshold} dict form.
_EntityValue = str | dict[str, str | float]


class TextTooLongError(ValueError):
    """Input exceeds the model's token limit (would be silently truncated)."""


class Engine:
    """Owns the loaded GLiNER2 model and the schema/result translation."""

    def __init__(self) -> None:
        self.model_id = config.model_id()
        self.max_tokens = config.max_tokens()
        self._model = _load(self.model_id)
        # The encoder tokenizer, used to enforce the token limit precisely (the
        # model silently truncates above it, so we must reject, not truncate).
        self._tokenizer = self._model.processor.tokenizer

    def count_tokens(self, text: str) -> int:
        return len(self._tokenizer.encode(text))

    def check_length(self, text: str) -> None:
        n = self.count_tokens(text)
        if n > self.max_tokens:
            raise TextTooLongError(
                f"input is {n} tokens; the limit is {self.max_tokens} "
                "(the model truncates above this, so it is rejected)"
            )

    def recognize(self, texts: list[str], schema: Schema, threshold: float) -> list[NerResponse]:
        """Run one batched extraction over ``texts`` sharing a single schema."""
        g_schema = build_schema(schema)
        results: list[_G2Result] = self._model.batch_extract(
            texts,
            g_schema,
            threshold=threshold,
            max_len=self.max_tokens,
            include_confidence=True,
            include_spans=True,
        )
        return [project(r, schema, self.model_id) for r in results]


def _load(model_id: str) -> GLiNER2:
    from gliner2 import GLiNER2

    kwargs: dict[str, bool] = {}
    if config.quantize():
        kwargs["quantize"] = True
    if config.compile_model():
        kwargs["compile"] = True
    return GLiNER2.from_pretrained(model_id, **kwargs)


def build_schema(schema: Schema) -> G2Schema:
    """Translate the wire schema into a ``gliner2.Schema``."""
    from gliner2 import RegexValidator
    from gliner2 import Schema as G2Schema

    g = G2Schema()

    if schema.entities:
        # gliner2 accepts {label: description} or {label: {description, threshold}}.
        # Use the dict form per label only when a threshold is set, so a bare
        # label stays a bare label.
        g.entities({e.label: _entity_value(e) for e in schema.entities})

    for c in schema.classifications:
        if c.threshold is None:
            g.classification(c.task, c.labels, multi_label=c.multi_label)
        else:
            g.classification(c.task, c.labels, multi_label=c.multi_label, cls_threshold=c.threshold)

    for s in schema.structures:
        builder = g.structure(s.name)
        for f in s.fields:
            validators: list[RegexValidator] | None = (
                [RegexValidator(f.pattern)] if f.pattern else None
            )
            builder.field(
                f.name,
                dtype=f.dtype,
                choices=f.choices,
                description=f.description,
                threshold=f.threshold,
                validators=validators,
            )

    return g


def _entity_value(spec: EntitySpec) -> _EntityValue:
    """The gliner2 entity-dict value for an ``EntitySpec``.

    The description string (``""`` for a bare label) when no threshold is set;
    otherwise the ``{description?, threshold}`` dict form gliner2 accepts.
    """
    if spec.threshold is None:
        return spec.description or ""
    value: dict[str, str | float] = {"threshold": spec.threshold}
    if spec.description is not None:
        value["description"] = spec.description
    return value


def project(result: _G2Result, schema: Schema, model_id: str) -> NerResponse:
    """Map a gliner2 ``batch_extract`` result dict to a typed response.

    The result is keyed by task: ``"entities"`` (dict label -> spans), each
    classification task name, and each structure name. We route by the
    *schema's* declared task types rather than sniffing the value shape — an
    empty classification and an empty structure both come back as ``[]``, so
    shape alone is ambiguous. gliner2's ``confidence`` becomes ``score``.
    """
    # Routing is by the schema's declared task types, so each cast below is
    # justified: gliner2 returns entities under "entities", a class result under
    # each classification task name, and a record list under each structure name.
    entity_groups = cast("dict[str, list[_G2Span]]", result.get("entities", {}))
    entities: list[Entity] = [
        Entity(
            text=s["text"],
            label=label,
            score=float(s["confidence"]),
            start=int(s["start"]),
            end=int(s["end"]),
        )
        for label, spans in entity_groups.items()
        for s in spans
    ]

    classifications: dict[str, Classification] = {
        c.task: _project_classification(cast("_G2ClassLabel | list[_G2ClassLabel]", result[c.task]))
        for c in schema.classifications
        if c.task in result
    }
    structures: dict[str, list[Record]] = {
        s.name: [
            _project_record(rec) for rec in cast("list[dict[str, list[_G2Span]]]", result[s.name])
        ]
        for s in schema.structures
        if s.name in result
    }

    return NerResponse(
        entities=entities,
        classifications=classifications,
        structures=structures,
        model_id=model_id,
    )


def _project_classification(value: _G2ClassLabel | list[_G2ClassLabel]) -> Classification:
    # Single-label is a {label, confidence} dict; multi-label is a list of them.
    if isinstance(value, list):
        return [ClassLabel(label=v["label"], score=float(v["confidence"])) for v in value]
    return ClassLabel(label=value["label"], score=float(value["confidence"]))


def _project_record(record: dict[str, list[_G2Span]]) -> Record:
    return {
        field: [
            Span(
                text=s["text"],
                score=float(s["confidence"]),
                start=int(s["start"]),
                end=int(s["end"]),
            )
            for s in spans
        ]
        for field, spans in record.items()
    }
