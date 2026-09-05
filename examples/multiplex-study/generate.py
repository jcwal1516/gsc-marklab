#!/usr/bin/env python3
"""Generate a deterministic synthetic grid cohort and verify its graph-count oracle.

These are software/performance fixtures, never biological or clinical validation data.
"""
import argparse
import json
import math
from pathlib import Path


def recipe(rows=2, columns=3):
    if rows < 2 or columns < 2:
        raise ValueError("grid dimensions must both be at least two")
    channels = [
        {"id":"CD3","label":"synthetic CD3 channel","kind":"continuous","unit":"fluorescence_au","measurement_status":"measured","provenance":"synthetic-grid-v1/continuous-1"},
        {"id":"CD8","label":"synthetic CD8 channel","kind":"continuous","unit":"fluorescence_au","measurement_status":"measured","provenance":"synthetic-grid-v1/continuous-2"},
        {"id":"phenotype","label":"synthetic binary mark","kind":"binary","unit":"unitless","measurement_status":"measured","provenance":"synthetic-grid-v1/binary"},
        {"id":"cell_class","label":"synthetic cell class","kind":"categorical","unit":"categorical","measurement_status":"imported_prediction","provenance":"synthetic-grid-v1/category","levels":["immune","tumor"]},
    ]
    slides = []
    for patient in range(6):
        for section in range(2):
            slide = f"p{patient}-s{section}"
            coordinates = [[x, y] for y in range(rows) for x in range(columns)]
            observations = {"CD3":[],"CD8":[],"phenotype":[],"cell_class":[]}
            for index, (x, y) in enumerate(coordinates):
                observations["CD3"].append(4 + math.sin((.21+.025*patient)*(x+1)) + math.cos((.19+.013*section)*(y+1)) + .07*((x*y+patient)%7))
                observations["CD8"].append(3 + math.cos((.31+.019*patient)*(y+1)) + math.sin((.23+.011*section)*(x+1)) + .05*((x+y+section)%5))
                observations["phenotype"].append(None if index % 13 == 0 else index % 2)
                observations["cell_class"].append(None if index % 17 == 0 else (index+patient)%2)
            slides.append({"slide_id":slide,"patient_id":f"p{patient}","group":"reference" if patient<3 else "comparison",
                "coordinate_frame_id":f"{slide}-um", "window":{"type":"MultiPolygon","coordinates":[[[[-1,-1],[columns,-1],[columns,rows],[-1,rows],[-1,-1]]]]},
                "cell_ids":[f"{slide}:c{index:07}" for index in range(len(coordinates))],
                "coordinates_um":coordinates,"observations":observations})
    return {"format":"marklab.multiplex_study_recipe","version":1,"study_id":"synthetic-grid-cohort",
        "channels":channels,"slides":slides,"design":{"selected_channels":["CD3","CD8"],"radius_um":1.1,
        "weight_policy":"binary_symmetric","missingness":"per_channel_complete_case","patient_reduction":"equal_slide_mean",
        "exchangeability":"independent_patients","group_a":"comparison","group_b":"reference","permutations":99,"seed":41,"alpha":.05}}


def verify(result, rows, columns):
    expected_edges = 4*rows*columns - 2*rows - 2*columns
    assert len(result["slides"]) == 12
    assert len(result["patients"]) == 6
    assert result["inference"]["status"] == "available"
    for slide in result["slides"]:
        assert slide["cell_count"] == rows*columns
        assert len(slide["channels"]) == 2
        for channel in slide["channels"]:
            assert channel["status"] == "available"
            assert channel["directed_edges"] == expected_edges
            assert channel["observed_rows"] == rows*columns
    for patient in result["patients"]:
        assert patient["slide_count"] == 2
    return {"total_cells":12*rows*columns,"edges_per_slide_channel":expected_edges,
            "reserved_statistic_edge_evaluations":12*2*2*expected_edges}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rows",type=int,default=2)
    parser.add_argument("--columns",type=int,default=3)
    parser.add_argument("--out",type=Path)
    parser.add_argument("--verify",type=Path)
    args = parser.parse_args()
    if args.verify:
        print(json.dumps(verify(json.loads(args.verify.read_text()),args.rows,args.columns),sort_keys=True))
    elif args.out:
        with args.out.open("x") as stream:
            json.dump(recipe(args.rows,args.columns),stream,separators=(",",":"),allow_nan=False)
            stream.write("\n")
    else:
        parser.error("supply --out or --verify")
