"""Unit tests for the docTR -> contract geometry mapping (pure functions)."""

from __future__ import annotations

from types import SimpleNamespace


def _word(value, conf, geom):
    return SimpleNamespace(value=value, confidence=conf, geometry=geom)


def _page(blocks, dims=(50, 100)):
    return SimpleNamespace(dimensions=dims, blocks=blocks)


def _block(lines, geom):
    return SimpleNamespace(geometry=geom, lines=lines)


def _line(words, geom):
    return SimpleNamespace(geometry=geom, words=words)


def test_two_point_geometry_no_polygon():
    from nvisy_ocr.service import _geom_to_geometry

    bbox, poly = _geom_to_geometry(((0.0, 0.0), (0.3, 0.1)), 100, 50)
    assert (bbox.x, bbox.y, bbox.width, bbox.height) == (0.0, 0.0, 30.0, 5.0)
    assert poly is None


def test_four_point_geometry_populates_polygon():
    from nvisy_ocr.service import _geom_to_geometry

    # rotated quad (assume_straight_pages=False): 4 normalized points.
    quad = ((0.0, 0.1), (0.4, 0.0), (0.4, 0.2), (0.0, 0.3))
    bbox, poly = _geom_to_geometry(quad, 100, 50)
    assert poly is not None and len(poly) == 4
    # bbox is the axis-aligned extent of the quad.
    assert (bbox.x, bbox.y) == (0.0, 0.0)
    assert (bbox.width, bbox.height) == (40.0, 15.0)


def test_multi_block_hierarchy_preserved():
    from nvisy_ocr.service import _page_from_doctr

    b1 = _block(
        [_line([_word("A", 0.9, ((0, 0), (0.1, 0.1)))], ((0, 0), (0.1, 0.1)))], ((0, 0), (0.1, 0.1))
    )
    b2 = _block(
        [_line([_word("B", 0.9, ((0.5, 0.5), (0.6, 0.6)))], ((0.5, 0.5), (0.6, 0.6)))],
        ((0.5, 0.5), (0.6, 0.6)),
    )
    page = _page_from_doctr(_page([b1, b2]), threshold=0.0)
    assert [blk.lines[0].words[0].text for blk in page.blocks] == ["A", "B"]


def test_confidence_filter_recomputes_line_bbox():
    from nvisy_ocr.service import _page_from_doctr

    keep = _word("keep", 0.9, ((0.0, 0.0), (0.2, 0.1)))
    drop = _word("drop", 0.1, ((0.0, 0.0), (0.9, 0.9)))  # would dominate the bbox
    line = _line([keep, drop], ((0.0, 0.0), (0.9, 0.9)))
    page = _page_from_doctr(_page([_block([line], ((0, 0), (0.9, 0.9)))]), threshold=0.5)
    ln = page.blocks[0].lines[0]
    assert [w.text for w in ln.words] == ["keep"]
    # line bbox is recomputed from kept words, not docTR's full-line box.
    assert ln.bbox.width == 20.0 and ln.bbox.height == 5.0


def test_empty_page_yields_no_blocks():
    from nvisy_ocr.service import _page_from_doctr

    page = _page_from_doctr(_page([]), threshold=0.0)
    assert page.blocks == []
