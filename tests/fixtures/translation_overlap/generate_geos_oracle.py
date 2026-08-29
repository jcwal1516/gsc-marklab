#!/usr/bin/env python3
"""Generate the independent GEOS translation-overlap oracle."""

import json
import re
import subprocess


WINDOW = (
    "MULTIPOLYGON (((0 0,10 0,10 10,0 10,0 0),"
    "(3 3,3 7,7 7,7 3,3 3)),((12 0,15 0,15 3,12 3,12 0)))"
)
TRANSLATED = (
    "MULTIPOLYGON (((1.25 -0.75,11.25 -0.75,11.25 9.25,1.25 9.25,1.25 -0.75),"
    "(4.25 2.25,4.25 6.25,8.25 6.25,8.25 2.25,4.25 2.25)),"
    "((13.25 -0.75,16.25 -0.75,16.25 2.25,13.25 2.25,13.25 -0.75)))"
)
WINDOW_GEOJSON = (
    '{"type":"MultiPolygon","coordinates":['
    '[[[0,0],[10,0],[10,10],[0,10],[0,0]],[[3,3],[3,7],[7,7],[7,3],[3,3]]],'
    '[[[12,0],[15,0],[15,3],[12,3],[12,0]]]]}'
)


def geosop(*arguments: str) -> str:
    return subprocess.run(
        ["geosop", *arguments], check=True, capture_output=True, text=True
    ).stdout.strip()


version = subprocess.run(
    ["geosop", "--help"], check=True, capture_output=True, text=True
)
match = re.search(r"geosop - GEOS ([^\n]+)", version.stderr + version.stdout)
if match is None:
    raise RuntimeError("GEOS version was not reported")
intersection = geosop("-a", WINDOW, "-b", TRANSLATED, "intersection")
result = {
    "displacement_um": [1.25, -0.75],
    "geos_version": match.group(1),
    "intersection_wkt": intersection,
    "oracle": "GEOS geosop",
    "translation_overlap_area_um2": float(geosop("-a", intersection, "area")),
    "window_area_um2": float(geosop("-a", WINDOW, "area")),
    "window_geojson": WINDOW_GEOJSON,
}
print(json.dumps(result, indent=2, sort_keys=True))
