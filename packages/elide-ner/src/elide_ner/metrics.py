"""Prometheus metrics for the NER service.

Separate from the service so the full set is visible in one place: BentoML
already reports request counts, durations and statuses, so everything here
exists to say something it cannot — which bound a refusal breached, and the
shape of the work a call carried.
"""

from __future__ import annotations

from prometheus_client import Counter, Histogram

# prometheus_client directly (bentoml.metrics is deprecated in 1.4); BentoML
# sets PROMETHEUS_MULTIPROC_DIR so this is multiprocess-safe across workers.
batch_size_metric = Histogram(
    "elide_ner_batch_size",
    "Number of texts merged into one recognize() call.",
    buckets=(1, 2, 4, 8, 16, 32),
)

# Each of the three bounds a request can breach, counted separately. A caller
# chunks to fit this service's limits, so a rejection means the caller's own
# budget disagrees with the service's — which is a bug in the chunker, not
# traffic to be tolerated, and it is invisible if all refusals look alike.
rejected_metric = Counter(
    "elide_ner_rejected_total",
    "Requests refused before inference, by which bound they breached.",
    labelnames=("reason",),
)

# Tokens per text, against the model's own limit. The useful shape is how
# close traffic runs to the cap: a pile-up just under it means the caller is
# chunking to this service's ceiling and any drift in either budget starts
# rejecting real work.
tokens_metric = Histogram(
    "elide_ner_input_tokens",
    "Encoder tokens per text, as counted when enforcing the token limit.",
    buckets=(16, 32, 64, 128, 256, 384, 448, 512),
)

# Labels per request: the other half of an extraction's cost, and the
# dimension that went unbounded long enough to OOM a worker.
labels_metric = Histogram(
    "elide_ner_schema_labels",
    "Entity labels requested per text.",
    buckets=(1, 2, 4, 8, 16, 32, 64),
)

# Entities returned, which is what a caller ultimately consumes. Zero for a
# whole batch is the signature of a schema whose labels do not match the text.
entities_metric = Histogram(
    "elide_ner_entities_found",
    "Entities returned per text.",
    buckets=(0, 1, 2, 4, 8, 16, 32, 64),
)
