"""Prometheus metrics for the OCR service."""

from prometheus_client import Counter, Histogram

batch_size_metric = Histogram(
    "elide_ocr_batch_size",
    "Number of images merged into one recognize() call.",
    buckets=(1, 2, 4, 8, 16),
)

# Which bound a refused request breached. BentoML already counts 400s; what it
# cannot say is whether the caller sent corrupt base64 or bytes that are not an
# image, and those point at different faults upstream.
rejected_metric = Counter(
    "elide_ocr_rejected_total",
    "Image payloads refused before inference, by which bound they breached.",
    labelnames=("reason",),
)

# Words returned per image. Zero across a batch is the signature of a blank or
# unreadable scan rather than a quiet success.
words_metric = Histogram(
    "elide_ocr_words_found",
    "Words returned per image.",
    buckets=(0, 1, 8, 32, 128, 512, 2048),
)
