"""Schema-grouped inference, independent of the HTTP framework."""

from __future__ import annotations

from collections import defaultdict

from elide_core.ner.v1 import NerRequest, NerResponse

from elide_ner.inference.backends.base import NerBackend
from elide_ner.inference.validation import (
    SchemaTooLargeError,
    TextTooLongError,
    schema_size,
)
from elide_ner.runtime.metrics import entities_metric, rejected_metric, schema_metric, tokens_metric


def schema_key(req: NerRequest) -> tuple[float, str]:
    """A hashable key identifying requests that can share one batched call.

    Two requests batch together iff they have the same schema and threshold. The
    schema is serialized canonically (field order is stable) so equal schemas
    produce equal JSON, hence equal keys.
    """
    return (req.threshold, req.schema_.model_dump_json())


class NerPipeline:
    def __init__(self, backend: NerBackend) -> None:
        self.backend = backend

    def recognize(self, requests: list[NerRequest]) -> tuple[list[NerResponse], int]:
        # Reject over-length inputs up front (the model silently truncates above
        # its token limit) — a clean 400 for the whole batch, before inference.
        # The check returns the count it measured, so usage reporting below
        # reuses this encode rather than tokenizing a second time.
        token_counts: list[int] = []
        for req in requests:
            size = schema_size(req.schema_)
            schema_metric.observe(size)
            try:
                self.backend.check_schema(size)
                tokens = self.backend.check_length(req.text)
            except SchemaTooLargeError:
                rejected_metric.labels(reason="schema").inc()
                raise
            except TextTooLongError:
                rejected_metric.labels(reason="tokens").inc()
                raise
            tokens_metric.observe(tokens)
            token_counts.append(tokens)

        # Group requests that share an identical schema + threshold so each group
        # is one batched extraction call. Distinct schemas run separately.
        groups: dict[tuple[float, str], list[int]] = defaultdict(list)
        for i, req in enumerate(requests):
            groups[schema_key(req)].append(i)

        by_index: dict[int, NerResponse] = {}
        for idxs in groups.values():
            head = requests[idxs[0]]
            responses = self.backend.recognize(
                [requests[i].text for i in idxs],
                head.schema_,
                threshold=head.threshold,
                token_counts=[token_counts[i] for i in idxs],
            )
            for i, resp in zip(idxs, responses, strict=True):
                by_index[i] = resp
                entities_metric.observe(len(resp.entities))
        return [by_index[i] for i in range(len(requests))], len(groups)
