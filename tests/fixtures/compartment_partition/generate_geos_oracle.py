#!/usr/bin/env python3
"""Generate the independent GEOS oracle for the aligned binary partition."""

import json
import re
import subprocess


DOMAIN = "POLYGON ((0 0, 5 0, 10 0, 10 10, 5 10, 0 10, 0 0))"
NEGATIVE = "POLYGON ((0 0, 5 0, 5 10, 0 10, 0 0))"
POSITIVE = "POLYGON ((5 0, 10 0, 10 10, 5 10, 5 0))"
INTERFACE = "LINESTRING (5 0, 5 10)"


def geosop(*args: str) -> str:
    completed = subprocess.run(
        ["geosop", *args],
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


def metric(geometry: str, operation: str) -> float:
    return float(geosop("-a", geometry, operation))


def distance(point: str) -> float:
    return float(geosop("-a", INTERFACE, "-b", point, "distance"))


help_result = subprocess.run(
    ["geosop", "--help"], check=True, capture_output=True, text=True
)
version = help_result.stderr + help_result.stdout
match = re.search(r"geosop - GEOS ([^\n]+)", version)
if match is None:
    raise RuntimeError("GEOS version was not reported")

union = geosop("-a", NEGATIVE, "-b", POSITIVE, "union")
intersection = geosop("-a", NEGATIVE, "-b", POSITIVE, "intersection")
result = {
    "oracle": "GEOS geosop",
    "geos_version": match.group(1),
    "negative_area_um2": metric(NEGATIVE, "area"),
    "positive_area_um2": metric(POSITIVE, "area"),
    "observation_area_um2": metric(DOMAIN, "area"),
    "union_equals_observation": geosop("-a", union, "-b", DOMAIN, "equals") == "true",
    "intersection_wkt": intersection,
    "interface_length_um": metric(intersection, "length"),
    "compartment_boundary_length_um": metric(NEGATIVE, "length"),
    "compartment_outer_boundary_length_um": metric(NEGATIVE, "length")
    - metric(intersection, "length"),
    "query_distances_um": {
        "stroma": distance("POINT (2 5)"),
        "tumor": distance("POINT (7 5)"),
        "tissue_edge": distance("POINT (0 5)"),
    },
}
print(json.dumps(result, indent=2, sort_keys=True))
