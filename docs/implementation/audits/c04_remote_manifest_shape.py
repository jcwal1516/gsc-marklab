#!/usr/bin/env python3
"""Emit aggregate-only C-04 source-manifest shape evidence."""

from __future__ import annotations

import collections
import json
import pathlib
import sys
import unicodedata


MAX_MANIFEST_BYTES = 64 * 1024
TOP_LEVEL_KEYS = {
    "block_um",
    "coordinate_unit",
    "deduplicated_cells",
    "embedding_dtype",
    "embedding_width",
    "n_cells",
    "row_alignment",
    "schema_name",
    "schema_version",
    "sources",
}
SOURCE_KEYS = {
    "cells_json_sha256",
    "cells_pt_sha256",
    "graph_position_scale",
    "mpp",
    "roi_id",
    "slide_path",
    "specimen_id",
    "timepoint",
    "tumor_mask",
}


def unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise ValueError("duplicate JSON object key")
        value[key] = item
    return value


def is_control_free(value: str) -> bool:
    return all(unicodedata.category(character) != "Cc" for character in value)


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("c04-manifest-audit-error:usage")
    root = pathlib.Path(sys.argv[1])
    paths = sorted(root.rglob("bundle_manifest.json"))
    if not paths:
        raise SystemExit("c04-manifest-audit-error:no-manifests")
    key_sets: collections.Counter[tuple[str, ...]] = collections.Counter()
    source_key_sets: collections.Counter[tuple[str, ...]] = collections.Counter()
    types: dict[str, collections.Counter[str]] = collections.defaultdict(collections.Counter)
    source_types: dict[str, collections.Counter[str]] = collections.defaultdict(
        collections.Counter
    )
    lengths: dict[str, list[int]] = collections.defaultdict(list)
    sizes: list[int] = []
    for path in paths:
        encoded_size = path.stat().st_size
        if encoded_size > MAX_MANIFEST_BYTES:
            raise SystemExit("c04-manifest-audit-error:file-budget")
        raw = path.read_bytes()
        if len(raw) != encoded_size or len(raw) > MAX_MANIFEST_BYTES:
            raise SystemExit("c04-manifest-audit-error:file-changed")
        sizes.append(len(raw))
        try:
            value = json.loads(raw, object_pairs_hook=unique_object)
        except (UnicodeDecodeError, json.JSONDecodeError, ValueError):
            raise SystemExit("c04-manifest-audit-error:json") from None
        if not isinstance(value, dict) or set(value) != TOP_LEVEL_KEYS:
            raise SystemExit("c04-manifest-audit-error:top-shape")
        key_sets[tuple(sorted(value))] += 1
        for key, item in value.items():
            types[key][type(item).__name__] += 1
            if isinstance(item, str):
                lengths[f"top.{key}"].append(len(item.encode("utf-8")))
        sources = value.get("sources")
        if not isinstance(sources, list) or len(sources) != 1:
            raise SystemExit("c04-manifest-audit-error:sources")
        for source in sources:
            if not isinstance(source, dict) or set(source) != SOURCE_KEYS:
                raise SystemExit("c04-manifest-audit-error:source-shape")
            source_key_sets[tuple(sorted(source))] += 1
            for key, item in source.items():
                source_types[key][type(item).__name__] += 1
                if isinstance(item, str):
                    if not is_control_free(item):
                        raise SystemExit("c04-manifest-audit-error:source-control")
                    lengths[f"source.{key}"].append(len(item.encode("utf-8")))
    report = {
        "bundles": len(paths),
        "format": "marklab.c04_remote_manifest_shape_audit",
        "maximum_bytes": max(sizes),
        "minimum_bytes": min(sizes),
        "string_byte_lengths": {
            key: {"maximum": max(values), "minimum": min(values)}
            for key, values in sorted(lengths.items())
        },
        "source_key_sets": [
            {"count": count, "keys": list(key_set)}
            for key_set, count in sorted(source_key_sets.items())
        ],
        "source_types": {
            key: dict(sorted(counts.items()))
            for key, counts in sorted(source_types.items())
        },
        "top_level_key_sets": [
            {"count": count, "keys": list(key_set)}
            for key_set, count in sorted(key_sets.items())
        ],
        "top_level_types": {
            key: dict(sorted(counts.items())) for key, counts in sorted(types.items())
        },
        "version": 1,
    }
    print(json.dumps(report, sort_keys=True, separators=(",", ":")))


if __name__ == "__main__":
    main()
