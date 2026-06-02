# nvisy-toolkit

Composable component library for Nvisy pipelines: recognizer registry,
deduplication layers, validation checks, redaction strategies. The
pieces a consumer composes when building their own document-processing
pipeline.

Sits between `nvisy-core` (atoms: primitives, Entity, Modality trait)
and `nvisy-document` (the whole-document runner that calls into these
components).
