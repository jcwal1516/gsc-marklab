#!/usr/bin/env python3
"""Generate the independent GEOS oracle for polygon fragmentation."""

import json
import math
import re
import subprocess


DOMAIN = "POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0))"
SMALL = "POLYGON ((1 1, 2 1, 2 2, 1 2, 1 1))"
LARGE = "POLYGON ((4 4, 6 4, 6 6, 4 6, 4 4))"
ISLANDS = (
    "MULTIPOLYGON (((1 1, 2 1, 2 2, 1 2, 1 1)), "
    "((4 4, 6 4, 6 6, 4 6, 4 4)))"
)
BACKGROUND = (
    "POLYGON ((0 0, 10 0, 10 10, 0 10, 0 0), "
    "(1 1, 2 1, 2 2, 1 2, 1 1), "
    "(4 4, 6 4, 6 6, 4 6, 4 4))"
)


def geosop(*args: str) -> str:
    completed = subprocess.run(
        ["geosop", *args], check=True, capture_output=True, text=True
    )
    return completed.stdout.strip()


def metric(geometry: str, operation: str) -> float:
    return float(geosop("-a", geometry, operation))


help_result = subprocess.run(
    ["geosop", "--help"], check=True, capture_output=True, text=True
)
version = help_result.stderr + help_result.stdout
match = re.search(r"geosop - GEOS ([^\n]+)", version)
if match is None:
    raise RuntimeError("GEOS version was not reported")

areas = [metric(SMALL, "area"), metric(LARGE, "area")]
total = sum(areas)
probabilities = [area / total for area in areas]
entropy = -sum(probability * math.log(probability) for probability in probabilities)
union = geosop("-a", ISLANDS, "-b", BACKGROUND, "union")
result = {
    "oracle": "GEOS geosop plus Python standard-library entropy",
    "geos_version": match.group(1),
    "domain_area_um2": metric(DOMAIN, "area"),
    "islands_area_um2": metric(ISLANDS, "area"),
    "islands_perimeter_um": metric(ISLANDS, "length"),
    "islands_component_areas_um2": areas,
    "islands_component_area_entropy_nats": entropy,
    "islands_normalized_component_area_entropy": entropy / math.log(len(areas)),
    "background_area_um2": metric(BACKGROUND, "area"),
    "background_perimeter_um": metric(BACKGROUND, "length"),
    "background_perimeter_area_ratio_per_um": metric(BACKGROUND, "length")
    / metric(BACKGROUND, "area"),
    "union_equals_domain": geosop("-a", union, "-b", DOMAIN, "equals") == "true",
}
print(json.dumps(result, indent=2, sort_keys=True))
