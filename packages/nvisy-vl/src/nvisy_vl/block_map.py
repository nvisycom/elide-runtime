"""Map PaddleOCR-VL's layout labels onto the canonical :class:`BlockKind`.

PaddleOCR-VL's layout model emits fine-grained ``block_label`` strings (title,
text, list, table, figure, formula, chart, header, footer, …). Our contract has
a small, stable :class:`BlockKind` (text/table/figure/other). This collapses the
former onto the latter; unknown labels fall back to ``OTHER``. Owning the map
here keeps the model swappable behind the contract (same pattern as the GLiNER
label map).
"""

from __future__ import annotations

from nvisy_core.ocr.v1 import BlockKind

# Known PaddleOCR-VL / PP-DocLayout labels -> BlockKind. Anything not listed
# maps to OTHER (see block_kind).
_LABEL_TO_KIND: dict[str, BlockKind] = {
    "text": BlockKind.TEXT,
    "title": BlockKind.TEXT,
    "paragraph_title": BlockKind.TEXT,
    "doc_title": BlockKind.TEXT,
    "abstract": BlockKind.TEXT,
    "content": BlockKind.TEXT,
    "list": BlockKind.TEXT,
    "reference": BlockKind.TEXT,
    "header": BlockKind.TEXT,
    "footer": BlockKind.TEXT,
    "footnote": BlockKind.TEXT,
    "table": BlockKind.TABLE,
    "table_caption": BlockKind.TABLE,
    "figure": BlockKind.FIGURE,
    "image": BlockKind.FIGURE,
    "figure_caption": BlockKind.FIGURE,
    "chart": BlockKind.FIGURE,
    "formula": BlockKind.OTHER,
    "seal": BlockKind.OTHER,
    "number": BlockKind.OTHER,
}


def block_kind(label: str) -> BlockKind:
    """Resolve a PaddleOCR-VL ``block_label`` to a :class:`BlockKind`."""
    return _LABEL_TO_KIND.get(label.lower().strip(), BlockKind.OTHER)
