#!/usr/bin/env python3
"""Generate the three synthetic pathology-study example recipes.

Sixteen patients with two sections each. Patients p00-p07 form group A, in which about 28% of
cells are immune; p08-p15 form group B, at about 14%. Cells are placed uniformly at random in
a 1000 um square, so there is a composition difference between the groups but no spatial
structure. These are software examples, never biological or clinical data.

Usage:  python examples/pathology-studies/generate.py --out-dir examples/pathology-studies
"""

import argparse
import json
import random
from pathlib import Path

SEED = 20261002
PATIENTS = 16
SECTIONS_PER_PATIENT = 2
WINDOW_UM = 1000
IMMUNE_FRACTION = {"A": 0.28, "B": 0.14}
LIMITS = {
    "maximum_cells": 10000,
    "maximum_work": 1000000000,
    "maximum_geometry_vertices": 10000,
    "maximum_tiles": 100000,
    "memory_budget_bytes": 536870912,
}


def require(condition, message):
    if not condition:
        raise ValueError(f"generated study breaks a recipe rule: {message}")


def study():
    rng = random.Random(SEED)
    window = [
        [[[0, 0], [WINDOW_UM, 0], [WINDOW_UM, WINDOW_UM], [0, WINDOW_UM], [0, 0]]]
    ]
    slides = []
    for patient in range(PATIENTS):
        group = "A" if patient < PATIENTS // 2 else "B"
        for section in range(SECTIONS_PER_PATIENT):
            cell_count = 220 + len(slides)
            cells = []
            for index in range(cell_count):
                cells.append(
                    {
                        "id": f"c{index:04d}",
                        "x_um": round(rng.uniform(0, WINDOW_UM), 3),
                        "y_um": round(rng.uniform(0, WINDOW_UM), 3),
                        "phenotype": (
                            "immune"
                            if rng.random() < IMMUNE_FRACTION[group]
                            else "tumor"
                        ),
                        "stratum": "all",
                    }
                )
            positions = {(cell["x_um"], cell["y_um"]) for cell in cells}
            require(
                len(positions) == cell_count,
                f"p{patient:02d}-s{section} repeats a position",
            )
            require(
                all(0 < x < WINDOW_UM and 0 < y < WINDOW_UM for x, y in positions),
                f"p{patient:02d}-s{section} has a cell on the window boundary",
            )
            slides.append(
                {
                    "slide_id": f"p{patient:02d}-s{section}",
                    "patient_id": f"p{patient:02d}",
                    "group": group,
                    "coordinate_frame_id": f"frame-{patient}-{section}",
                    "window": {"type": "MultiPolygon", "coordinates": window},
                    "cells": cells,
                }
            )
    return {
        "study_id": "synthetic-three-workflows",
        "phenotypes": {
            "names": ["immune", "tumor"],
            "measurement_status": "measured",
            "provenance": "synthetic labels; no measured biological cohort",
        },
        "slides": slides,
    }


def maup_grids():
    grids = []
    for width in (100, 200, 400):
        half = width / 2
        for suffix, offset in (
            ("00", [0.0, 0.0]),
            ("10", [half, 0.0]),
            ("01", [0.0, half]),
            ("11", [half, half]),
        ):
            grids.append(
                {"id": f"w{width}-{suffix}", "width_um": width, "offset_um": offset}
            )
    return grids


def designs():
    composition = {
        "group_a": "A",
        "group_b": "B",
        "permutations": 199,
        "bootstrap_replicates": 499,
        "seed": SEED,
        "alpha": 0.05,
        "balances": [
            {
                "name": "immune-vs-tumor",
                "kind": "amalgamated_log_ratio",
                "numerator": ["immune"],
                "denominator": ["tumor"],
            }
        ],
        "maup": {
            "phenotype": "immune",
            "origin_um": [0, 0],
            "primary": "w200-00",
            "grids": maup_grids(),
        },
    }
    spatial_study = {
        "group_a": "A",
        "group_b": "B",
        "permutations": 199,
        "seed": SEED,
        "alpha": 0.05,
        "statistic": "cross_g_minus_one",
        "phenotype": None,
        "source": "immune",
        "target": "tumor",
        "radii_um": [50, 75, 100],
        "pair_bandwidth_um": 25,
        "intensity": {
            "model": "gaussian",
            "bandwidth_um": 250,
            "grid": [12, 12],
            "minimum_intensity_per_um2": 1e-12,
            "cross_fit_folds": None,
            "bandwidth_candidates_um": [],
            "null_mode": "refit",
        },
        "compartments": [],
    }
    scan = {
        "positive_phenotype": "immune",
        "eligible_phenotypes": ["immune", "tumor"],
        "radii_um": [100, 200, 300],
        "permutations": 199,
        "seed": SEED,
        "alpha": 0.05,
    }
    return {
        "composition": ("marklab.pathology_composition_recipe", composition, LIMITS),
        "spatial-study": (
            "marklab.pathology_spatial_study_recipe",
            spatial_study,
            {**LIMITS, "maximum_work": 3000000000},
        ),
        "scan": ("marklab.pathology_scan_recipe", scan, LIMITS),
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out-dir", type=Path, required=True)
    args = parser.parse_args()
    shared = study()
    for name, (format_name, design, limits) in designs().items():
        document = {
            "format": format_name,
            "version": 1,
            "study": shared,
            "design": design,
            "limits": limits,
        }
        with (args.out_dir / f"{name}.json").open("w") as stream:
            json.dump(document, stream, separators=(",", ":"), allow_nan=False)
            stream.write("\n")
