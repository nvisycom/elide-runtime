"""Contract implemented by NER model backends."""

from __future__ import annotations

from typing import Protocol

from elide_core.ner.v1 import NerResponse, Schema


class NerBackend(Protocol):
    max_tokens: int

    def check_schema(self, size: int) -> None: ...

    def check_length(self, text: str) -> int: ...

    def recognize(
        self,
        texts: list[str],
        schema: Schema,
        threshold: float,
        token_counts: list[int] | None = None,
    ) -> list[NerResponse]: ...
