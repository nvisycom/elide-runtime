"""HTTP envelopes for the authoritative Python NER v1 contract."""

from pydantic import RootModel

from elide_core.ner.v1.base import _Model
from elide_core.ner.v1.request import NerRequest
from elide_core.ner.v1.response import NerResponse


class NerInput(_Model):
    requests: list[NerRequest]


class NerOutput(RootModel[list[NerResponse]]):
    pass
