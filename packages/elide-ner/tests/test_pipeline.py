"""Exercise batching without an HTTP server or model weights."""

from elide_core.ner.v1 import EntitySpec, NerRequest, NerResponse, Schema, TokenUsage
from elide_ner.inference.pipeline import NerPipeline


def test_pipeline_groups_by_schema_and_threshold_and_restores_order():
    class Backend:
        max_tokens = 512

        def __init__(self):
            self.calls = []

        def check_schema(self, size):
            pass

        def check_length(self, text):
            return len(text)

        def recognize(self, texts, schema, threshold, token_counts):
            self.calls.append((texts, schema.entities[0].label, threshold, token_counts))
            return [
                NerResponse(model_id=text, entities=[], tokens=TokenUsage(input=n, limit=512))
                for text, n in zip(texts, token_counts, strict=True)
            ]

    backend = Backend()
    person = Schema(entities=[EntitySpec(label="person")])
    place = Schema(entities=[EntitySpec(label="place")])
    requests = [
        NerRequest(text="Ada", schema=person, threshold=0.5),
        NerRequest(text="Rome", schema=place, threshold=0.5),
        NerRequest(text="Alan", schema=person, threshold=0.5),
        NerRequest(text="Grace", schema=person, threshold=0.9),
    ]

    responses, groups = NerPipeline(backend).recognize(requests)

    assert groups == 3
    assert backend.calls == [
        (["Ada", "Alan"], "person", 0.5, [3, 4]),
        (["Rome"], "place", 0.5, [4]),
        (["Grace"], "person", 0.9, [5]),
    ]
    assert [response.model_id for response in responses] == ["Ada", "Rome", "Alan", "Grace"]
    assert [response.tokens.input for response in responses] == [3, 4, 4, 5]
