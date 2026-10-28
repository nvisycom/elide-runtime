"""Prometheus metrics for the STT service."""

from prometheus_client import Counter, Histogram

# Which bound a refused request breached. BentoML counts 400s; it cannot say
# whether the payload was oversized, malformed, empty, or simply too long to
# transcribe — and those point at different faults upstream.
rejected_metric = Counter(
    "elide_stt_rejected_total",
    "Requests refused before inference, by which bound they breached.",
    labelnames=("reason",),
)

duration_metric = Histogram(
    "elide_stt_audio_seconds",
    "Duration of audio submitted to one transcribe() call.",
    buckets=(5, 15, 30, 60, 300, 900, 3600),
)
