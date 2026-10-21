#!/usr/bin/env python3
"""Download the openly licensed Schürch CODEX cell table into a fresh external directory.

Source: Schürch, C. M. et al. (2020), doi:10.17632/mpjzbtfgfr.1, CC BY 4.0.
The published cell table is preserved byte-for-byte; no source code is downloaded or run.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import subprocess
import sys
import urllib.request
import xml.etree.ElementTree as ET
import zipfile
from collections import Counter, defaultdict
from importlib.metadata import version
from pathlib import Path

DATASET = "https://data.mendeley.com/datasets/mpjzbtfgfr/1"
FILES = (
    "https://data.mendeley.com/public-api/datasets/mpjzbtfgfr/files"
    "?folder_id=root&version=1&$start=0&$limit=1000"
)
FILENAME = "CRC_clusters_neighborhoods_markers.csv"
MAXIMUM_DOWNLOAD_BYTES = 256 * 1024 * 1024
HEADERS = {
    "User-Agent": "Marklab-public-data-validation",
    "Accept": "application/vnd.mendeley-public-dataset.1+json",
}
METADATA = {
    "CRC_TMAs_patient_annotations.xlsx": (
        "https://www.cancerimagingarchive.net/wp-content/uploads/CRC_TMAs_patient_annotations.xlsx"
    ),
    "Details_on_image_acquisition_and_processing.docx": (
        "https://www.cancerimagingarchive.net/wp-content/uploads/Details_on_image_acquisition_and_processing.docx"
    ),
}
# Fixed before reference or native comparisons, not tuned to observed p-values.
DESIGN = {
    "groups": {"1": "CLR", "2": "DII"},
    "neighborhoods": {
        "1": "T cell enriched", "2": "Bulk tumor", "3": "Immune-infiltrated stroma",
        "4": "Macrophage enriched", "5": "Follicle", "6": "Tumor boundary",
        "7": "Vascularized smooth muscle", "8": "Smooth muscle", "9": "Granulocyte enriched",
    },
    "patient_reduction_composition": "pool all A/B cores; fraction of cells with a published final neighborhood",
    "multiplex_tma": "A",
    "multiplex_excluded_cell_type": "dirt",
    "micrometres_per_pixel": 0.377442,
    "image_pixels": [1920, 1440],
    "radius_um": 100.0,
    "weight_policy": "binary_symmetric_including_isolates",
    "policy_amendment": "Explicit all-observed-row binary weights after strict-policy support failure; no cell, radius or tolerance changed",
    "marker_columns": {
        "CD3": "CD3 - T cells:Cyc_16_ch_4",
        "CD20": "CD20 - B cells:Cyc_8_ch_3",
    },
    "permutations": 19999,
    "seed": 20261010,
    "reference_seed": 20261011,
    "alpha": 0.05,
}
# Both adjusted p-values are independent Monte Carlo estimates of the same quantity.
# Their difference is compared with Z_LIMIT standard errors of a difference of two
# binomial proportions, plus one permutation step on each side for discreteness.
# This replaced a fixed 0.03 absolute limit after review on 2026-10-10: near
# alpha=0.05 that limit admitted errors large enough to change the decision.
TOLERANCES = {
    "effect_absolute": 1e-12,
    "studentized_absolute": 1e-10,
    "independent_mc_adjusted_p_z": 4.0,
    "moran_geary_absolute": 1e-10,
}


def monte_carlo_p_limit(first: float, second: float, first_draws: int, second_draws: int) -> float:
    """Allowed difference between two independent Monte Carlo p-value estimates."""
    p = (first + second) / 2
    steps = 1 / (first_draws + 1) + 1 / (second_draws + 1)
    return TOLERANCES["independent_mc_adjusted_p_z"] * math.sqrt(p * (1 - p) * steps) + steps


def request(url: str):
    return urllib.request.urlopen(
        urllib.request.Request(url, headers=HEADERS), timeout=60
    )


def download(directory: Path) -> Path:
    """Check the publisher's license, then preserve the original bounded CSV."""
    repository = Path(__file__).resolve().parents[2]
    directory = directory.resolve()
    if directory == repository or repository in directory.parents:
        raise ValueError("public cohort data must remain outside the checkout")
    if directory.exists():
        raise ValueError("download directory must be new; raw inputs are never replaced")
    with request(DATASET) as response:
        page = response.read(2 * 1024 * 1024 + 1).decode("utf-8")
    prefix = "window.INITIAL_STATE = "
    if prefix not in page:
        raise ValueError("publisher metadata is unavailable; do not infer the license")
    state, _ = json.JSONDecoder().raw_decode(page.split(prefix, 1)[1])
    snapshot = state["dataset"]["snapshot"]
    if snapshot["id"] != "mpjzbtfgfr" or snapshot["version"] != 1:
        raise ValueError("unexpected dataset/version")
    if snapshot["licence"]["short_name"] != "CC BY 4.0":
        raise ValueError("source no longer declares the reviewed open license")
    with request(FILES) as response:
        files = json.load(response)
    matches = [file for file in files if file["filename"] == FILENAME]
    if len(matches) != 1 or matches[0]["status"] != "COMPLETED":
        raise ValueError("published cell table is missing or ambiguous")
    details = matches[0]["content_details"]
    if not 0 < details["size"] <= MAXIMUM_DOWNLOAD_BYTES:
        raise ValueError("cell table exceeds the bounded download profile")
    directory.mkdir(parents=True)
    source = {
        "dataset": DATASET,
        "doi": snapshot["doi"],
        "title": snapshot["name"],
        "license": snapshot["licence"]["short_name"],
        "license_url": snapshot["licence"]["url"],
        "attribution": "Schürch, Christian M. and colleagues (2020)",
        "filename": FILENAME,
        "download_url": details["download_url"],
        "expected_bytes": details["size"],
    }
    (directory / "source.json").write_text(json.dumps(source, indent=2) + "\n")
    path = directory / FILENAME
    count = 0
    with request(details["download_url"]) as response, path.open("xb") as stream:
        while chunk := response.read(1024 * 1024):
            count += len(chunk)
            if count > details["size"]:
                raise ValueError("download exceeds publisher's declared file size")
            stream.write(chunk)
    if count != details["size"]:
        raise ValueError(f"incomplete download: {count} of {details['size']} bytes")
    for name, url in METADATA.items():
        with request(url) as response:
            content = response.read(5 * 1024 * 1024 + 1)
        if len(content) > 5 * 1024 * 1024:
            raise ValueError("source metadata exceeds 5 MiB")
        with (directory / name).open("xb") as stream:
            stream.write(content)
    print(f"Downloaded {path} ({count} bytes); CC BY 4.0", flush=True)
    return path


def patient_groups(directory: Path) -> dict[str, str]:
    """Read only deidentified IDs/group and histologic group definition from TCIA."""
    ns = {"s": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}
    with zipfile.ZipFile(directory / "CRC_TMAs_patient_annotations.xlsx") as archive:
        strings = ["".join(el.itertext()) for el in ET.fromstring(archive.read("xl/sharedStrings.xml"))]
        sheet = ET.fromstring(archive.read("xl/worksheets/sheet1.xml"))
    groups = {}
    for row in sheet.findall("s:sheetData/s:row", ns)[1:]:
        values = {}
        for cell in row:
            column = cell.attrib["r"].rstrip("0123456789")
            if column not in {"A", "B", "I"}:
                continue
            value = cell.find("s:v", ns)
            if value is not None:
                values[column] = strings[int(value.text)] if cell.attrib.get("t") == "s" else value.text
        if not values.get("A", "").isdigit():
            continue
        if values["B"] not in DESIGN["groups"] or values["I"] != {"1": "2", "2": "0"}[values["B"]]:
            raise ValueError("published group disagrees with CLR Graham-Appelman definition")
        if values["A"] in groups:
            raise ValueError("duplicate patient metadata")
        groups[values["A"]] = DESIGN["groups"][values["B"]]
    if Counter(groups.values()) != {"CLR": 17, "DII": 18}:
        raise ValueError("unexpected patient groups; no inferred or substituted patients")
    return groups


def prepare(directory: Path):
    from marklab_client import cell_table_slide

    groups = patient_groups(directory)
    counts = {patient: Counter() for patient in groups}
    regions, images, excluded = defaultdict(set), defaultdict(list), Counter()
    total = 0
    columns = DESIGN["marker_columns"]
    seen = set()
    with (directory / FILENAME).open(newline="") as stream:
        for row in csv.DictReader(stream):
            total += 1
            patient = row["patients"]
            if patient not in groups or DESIGN["groups"][row["groups"]] != groups[patient]:
                raise ValueError("cell patient/group disagrees with published metadata")
            if row["CellID"] in seen:
                raise ValueError("duplicate source CellID")
            seen.add(row["CellID"])
            image = f"{row['TMA_AB']}-{row['Region']}"
            regions[patient].add(image)
            neighborhood = row["neighborhood number final"]
            if neighborhood in DESIGN["neighborhoods"]:
                if row["neighborhood name"] != DESIGN["neighborhoods"][neighborhood]:
                    raise ValueError("source neighborhood codebook changed")
                counts[patient][f"CN{neighborhood}"] += 1
            elif neighborhood == "N/A" and row["neighborhood name"] == "":
                excluded["missing_final_neighborhood"] += 1
            else:
                raise ValueError("unexpected neighborhood; do not silently recode")
            if row["TMA_AB"] != DESIGN["multiplex_tma"]:
                excluded["multiplex_other_tma"] += 1
                continue
            if row["ClusterName"] == DESIGN["multiplex_excluded_cell_type"]:
                excluded["multiplex_dirt"] += 1
                continue
            images[(image, patient)].append({
                key: row[key] for key in ["CellID", "X:X", "Y:Y", *columns.values()]
            })
    if total != 258385 or len(regions) != 35 or any(len(value) != 4 for value in regions.values()):
        raise ValueError("expected 258385 cells and four A/B cores for each of 35 patients")
    channels = [
        dict(id=name, label=name, kind="continuous", unit="fluorescence_au",
             measurement_status="measured", provenance=f"Schürch CODEX original compensated CSV; {column}")
        for name, column in columns.items()
    ]
    scale = DESIGN["micrometres_per_pixel"]
    width, height = DESIGN["image_pixels"]
    left, bottom, right, top = -0.5 * scale, -0.5 * scale, (width - 0.5) * scale, (height - 0.5) * scale
    window = {"type": "MultiPolygon", "coordinates": [[[
        [left, bottom], [right, bottom], [right, top], [left, top], [left, bottom]
    ]]]}
    slides = []
    for (image, patient), rows in sorted(images.items()):
        if any(not (0 <= float(row["X:X"]) < width and 0 <= float(row["Y:Y"]) < height) for row in rows):
            raise ValueError("cell lies outside the published image dimensions")
        slides.append(cell_table_slide(
            rows, slide_id=image, patient_id=f"p{int(patient):02d}", group=groups[patient],
            coordinate_frame_id=image + "-um", window=window, channels=channels,
            cell_id_column="CellID", x_column="X:X", y_column="Y:Y",
            coordinate_scale_um=(scale, scale), channel_columns=columns,
        ))
    if len(slides) != 70 or any(n != 2 for n in Counter(slide["patient_id"] for slide in slides).values()):
        raise ValueError("multiplex subset must retain two TMA-A images per patient")
    recipe = {
        "format": "marklab.multiplex_study_recipe", "version": 1,
        "study_id": "schurch-2020-tma-a-cd3-cd20", "channels": channels, "slides": slides,
        "design": dict(selected_channels=list(columns), radius_um=DESIGN["radius_um"],
                       weight_policy=DESIGN["weight_policy"], missingness="per_channel_complete_case",
                       patient_reduction="equal_slide_mean", exchangeability="independent_patients",
                       group_a="CLR", group_b="DII", permutations=DESIGN["permutations"],
                       seed=DESIGN["seed"], alpha=DESIGN["alpha"]),
        "limits": dict(maximum_rows_per_slide=10000, maximum_total_rows=200000,
                       maximum_directed_edges=4000000, maximum_edge_evaluations=100000000,
                       maximum_memory_bytes=1073741824),
    }
    fractions = {f"p{int(patient):02d}": [counts[patient][f"CN{i}"] / counts[patient].total() for i in range(1, 10)] for patient in groups}
    return recipe, fractions, {f"p{int(p):02d}": group for p, group in groups.items()}, {
        "source_cells": total, "source_images": sum(map(len, regions.values())),
        "patients": 35, "groups": dict(Counter(groups.values())),
        "multiplex_cells": sum(len(slide["cell_ids"]) for slide in slides),
        "exclusions": dict(excluded),
    }


def compare_max_t(values, groups, endpoints, native):
    import numpy as np
    from scipy import stats

    a = np.array([values[p] for p in sorted(values) if groups[p] == "CLR"])
    b = np.array([values[p] for p in sorted(values) if groups[p] == "DII"])
    if a.shape[0] != 17 or b.shape[0] != 18:
        raise ValueError("reference must use all 35 independent patients")
    # SciPy's default two-sided tail is retained as a descriptive comparison.
    # Max-T uses the two-sided absolute statistic and one maximum per joint shuffle.
    def welch(left, right, axis=0):
        return stats.ttest_ind(left, right, axis=axis, equal_var=False).statistic

    reference = stats.permutation_test(
        (a, b), welch, n_resamples=DESIGN["permutations"], batch=128,
        vectorized=True, alternative="two-sided", rng=DESIGN["reference_seed"], axis=0,
    )
    maxima = np.max(np.abs(reference.null_distribution), axis=1)
    reference_adjusted = (1 + (maxima[:, None] >= np.abs(reference.statistic)).sum(axis=0)) / (len(maxima) + 1)
    if not np.all(np.isfinite(reference.null_distribution)):
        raise ValueError("non-finite reference randomization statistic")
    native_by_name = {row["endpoint"]: row for row in native}
    if set(native_by_name) != set(endpoints) or len(native) != len(endpoints):
        raise ValueError("endpoint family differs")
    effect = a.mean(axis=0) - b.mean(axis=0)
    rows = []
    for i, endpoint in enumerate(endpoints):
        row = native_by_name[endpoint]
        errors = {
            "effect_absolute": abs(row["effect_group_a_minus_group_b"] - effect[i]),
            "studentized_absolute": abs(row["studentized_statistic"] - reference.statistic[i]),
            "independent_mc_adjusted_p_absolute": abs(row["adjusted_p_value"] - reference_adjusted[i]),
        }
        limits = {
            "effect_absolute": TOLERANCES["effect_absolute"],
            "studentized_absolute": TOLERANCES["studentized_absolute"],
            "independent_mc_adjusted_p_absolute": monte_carlo_p_limit(
                row["adjusted_p_value"], float(reference_adjusted[i]),
                DESIGN["permutations"], DESIGN["permutations"],
            ),
        }
        rows.append(dict(endpoint=endpoint, native=row, reference_effect=float(effect[i]),
                         reference_studentized=float(reference.statistic[i]),
                         reference_adjusted_p=float(reference_adjusted[i]),
                         scipy_default_two_sided_unadjusted_p=float(reference.pvalue[i]),
                         errors={key: float(value) for key, value in errors.items()},
                         limits=limits,
                         passed=bool(all(value <= limits[key] for key, value in errors.items()))))
    return rows


def compare_spatial(recipe, result):
    import numpy as np
    from scipy.spatial import cKDTree

    results = {slide["slide_id"]: slide for slide in result["slides"]}
    patient_values = defaultdict(list)
    worst = 0.0
    for slide in recipe["slides"]:
        coordinates = np.asarray(slide["coordinates_um"])
        pairs = cKDTree(coordinates).query_pairs(DESIGN["radius_um"], output_type="ndarray")
        if not len(pairs):
            raise ValueError("declared graph has no edges; no slide may be dropped")
        native = {channel["channel"]: channel for channel in results[slide["slide_id"]]["channels"]}
        summaries = {}
        for name in DESIGN["marker_columns"]:
            values = np.asarray(slide["observations"][name], dtype=float)
            centered = values - values.mean()
            denominator = np.dot(centered, centered)
            moran = len(values) / len(pairs) * np.dot(centered[pairs[:, 0]], centered[pairs[:, 1]]) / denominator
            differences = values[pairs[:, 0]] - values[pairs[:, 1]]
            geary = (len(values) - 1) / (2 * len(pairs)) * np.dot(differences, differences) / denominator
            row = native[name]
            if row["status"] != "available" or row["directed_edges"] != 2 * len(pairs):
                raise ValueError("native graph support/edge count disagrees with SciPy reference")
            worst = max(worst, abs(row["moran_i"] - moran), abs(row["geary_c"] - geary))
            summaries[f"{name}:moran_i"] = float(moran)
            summaries[f"{name}:geary_c"] = float(geary)
        patient_values[slide["patient_id"]].append([summaries[name] for name in result["endpoint_names"]])
    return float(worst), {patient: np.mean(values, axis=0).tolist() for patient, values in patient_values.items()}


def run_patient_test(binary, output, name, values, groups, endpoints):
    table = output / f"{name}-patients.csv"
    with table.open("x", newline="") as stream:
        writer = csv.writer(stream)
        writer.writerow(["patient_id", "group", "endpoint", "value"])
        for patient in sorted(values):
            writer.writerows([patient, groups[patient], endpoint, value] for endpoint, value in zip(endpoints, values[patient], strict=True))
    result_path = output / f"{name}-max-t.json"
    subprocess.run([str(binary), "cohort", "max-t", "--input", str(table), "--group-a", "CLR",
                    "--group-b", "DII", "--permutations", str(DESIGN["permutations"]),
                    "--seed", str(DESIGN["seed"]), "--alpha", str(DESIGN["alpha"]), "--out", str(result_path)], check=True)
    native = json.loads(result_path.read_text())
    if native["groups"]["group_a_patients"] != 17 or native["groups"]["group_b_patients"] != 18:
        raise ValueError("native cohort changed the patient denominator")
    return native["endpoints"]


def compare_h5ad(slide, channels, output):
    """Round-trip a bounded real-cohort ROI, without claiming an upstream H5AD export."""
    import anndata
    import numpy as np
    import pandas as pd
    from scipy.sparse import csr_matrix
    from marklab_client import anndata_slide

    prefix = f"{len(slide['slide_id'].encode('utf-8'))}:{slide['slide_id']}:"
    if any(not identity.startswith(prefix) for identity in slide["cell_ids"]):
        raise ValueError("slide-qualified identities do not retain the declared source prefix")
    ids = [identity[len(prefix):] for identity in slide["cell_ids"]]
    matrix = np.asarray([
        [slide["observations"][channel["id"]][row] for channel in channels]
        for row in range(len(ids))
    ], dtype=float)
    adata = anndata.AnnData(
        csr_matrix(matrix), obs=pd.DataFrame(index=ids),
        var=pd.DataFrame(index=[channel["id"] for channel in channels]),
    )
    adata.obsm["spatial_um"] = np.asarray(slide["coordinates_um"], dtype=float)
    adata.uns["coordinate_unit"] = "micrometer"
    adata.uns["source_dataset"] = DATASET
    path = output / f"schurch-{slide['slide_id']}-derived.h5ad"
    if path.exists():
        raise ValueError("derived H5AD output already exists")
    with anndata.settings.override(allow_write_nullable_strings=True):
        adata.write_h5ad(path)
    loaded = anndata.read_h5ad(path)
    if loaded.uns["coordinate_unit"] != "micrometer" or loaded.uns["source_dataset"] != DATASET:
        raise ValueError("H5AD changed explicit calibration/source metadata")
    imported = anndata_slide(
        loaded, **{key: slide[key] for key in [
            "slide_id", "patient_id", "group", "coordinate_frame_id", "window",
        ]}, channels=channels, coordinate_unit="micrometer", spatial_key="spatial_um",
    )
    if imported != slide:
        raise ValueError("real-cohort H5AD changed an identity, coordinate, marker, group or window")
    comparison = dict(source=DATASET, slide_id=slide["slide_id"], cells=len(ids),
                      channels=[channel["id"] for channel in channels],
                      anndata_version=version("anndata"), exact_round_trip=True,
                      scope="H5AD derived from observed Schurch cells; not an upstream-provided H5AD")
    (output / "h5ad-comparison.json").write_text(json.dumps(comparison, indent=2) + "\n")
    return comparison


def validate(directory: Path, output: Path, binary: Path) -> None:
    import numpy as np
    import scipy
    from marklab_client import run_study

    repository = Path(__file__).resolve().parents[2]
    if any(path == repository or repository in path.parents for path in (directory, output)):
        raise ValueError("public cohort raw data and results must remain outside the checkout")
    output.mkdir(parents=True, exist_ok=False)
    (output / "design.json").write_text(json.dumps({"design": DESIGN, "tolerances": TOLERANCES}, indent=2) + "\n")
    recipe, fractions, groups, counts = prepare(directory)
    h5ad_comparison = compare_h5ad(recipe["slides"][0], recipe["channels"], output)
    (output / "input-summary.json").write_text(json.dumps(counts, indent=2) + "\n")
    print(f"Prepared {counts}; running native patient Max-T", flush=True)
    endpoints = [f"CN{i}" for i in range(1, 10)]
    native = run_patient_test(binary, output, "neighborhood", fractions, groups, endpoints)
    cohort_rows = compare_max_t(fractions, groups, endpoints, native)
    (output / "cohort-comparison.json").write_text(json.dumps(cohort_rows, indent=2) + "\n")
    # A combined 70-image run exceeds the native 100-million-edge-work cap.
    # Partition by each patient's two existing cores, then average both patient
    # vectors before ONE joint four-endpoint Max-T test. No cells/radii are changed.
    by_patient = defaultdict(list)
    for slide in recipe["slides"]:
        by_patient[slide["patient_id"]].append(slide)
    native_vectors, reference_vectors = defaultdict(list), defaultdict(list)
    spatial_error, study_endpoints = 0.0, None
    for index in range(2):
        label = f"core{index + 1}"
        part = {**recipe, "study_id": recipe["study_id"] + "-" + label,
                "slides": [by_patient[p][index] for p in sorted(by_patient)]}
        (output / f"{label}-recipe.json").write_text(json.dumps(part, separators=(",", ":")) + "\n")
        print(f"Running {label} multiplex study: all 35 patients, 35 sampled fields", flush=True)
        study = run_study(part, project=output / f"{label}-project", out=output / label, binary=binary, timeout_seconds=1800)
        if study["inference"]["status"] != "available":
            raise ValueError(f"prespecified multiplex inference unavailable: {study['inference']['reason']}")
        if study_endpoints is not None and study["endpoint_names"] != study_endpoints:
            raise ValueError("core runs used different endpoint axes")
        study_endpoints = study["endpoint_names"]
        error, patient_values = compare_spatial(part, study)
        spatial_error = max(spatial_error, error)
        if set(patient_values) != set(groups) or {p["patient_id"] for p in study["patients"]} != set(groups):
            raise ValueError("a core run lost or added a patient")
        for patient in study["patients"]:
            if patient["slide_count"] != 1 or patient["group"] != groups[patient["patient_id"]]:
                raise ValueError("a core run changed patient membership")
            native_vectors[patient["patient_id"]].append(patient["values"])
        for patient, values in patient_values.items():
            reference_vectors[patient].append(values)
    native_values = {p: np.mean(v, axis=0).tolist() for p, v in native_vectors.items()}
    reference_values = {p: np.mean(v, axis=0).tolist() for p, v in reference_vectors.items()}
    native_study = run_patient_test(binary, output, "multiplex", native_values, groups, study_endpoints)
    study_rows = compare_max_t(reference_values, groups, study_endpoints, native_study)
    summary = dict(
        source=DATASET, license="CC BY 4.0", counts=counts,
        reference=dict(numpy=np.__version__, scipy=scipy.__version__,
                       methods="SciPy cKDTree, direct Moran/Geary formula, ttest_ind and permutation_test"),
        design=DESIGN, tolerances=TOLERANCES,
        execution="Two 35-field study runs; equal-core patient mean; one joint four-endpoint cohort Max-T",
        cohort=cohort_rows, multiplex=study_rows, maximum_spatial_error=spatial_error,
        h5ad=h5ad_comparison,
        passed=all(row["passed"] for row in cohort_rows + study_rows) and spatial_error <= TOLERANCES["moran_geary_absolute"],
        limitations=[
            "Two separate prespecified Max-T families, not one joint family across both workflows",
            "Known CLR/DII sampling contrast; follicle enrichment is not independent biological discovery",
            "Supplied final neighborhoods are imported, not recomputed or validated by Marklab",
            "Image rectangle is sampled field, not tissue mask; no tissue-area density or point-process inference",
            "One TMA used for multiplex size bounds; both TMAs used for patient neighborhood fractions",
            "No survival, causal, diagnostic, whole-slide or calibration claim",
        ],
    )
    (output / "summary.json").write_text(json.dumps(summary, indent=2, allow_nan=False) + "\n")
    print(f"Passed: {summary['passed']}; spatial error {spatial_error:.6g}; results {output}", flush=True)
    if not summary["passed"]:
        raise ValueError("reference comparison failed; tolerances are unchanged")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--download", type=Path)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/marklab"))
    args = parser.parse_args()
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "clients/python"))
    if args.download:
        if args.source or args.out:
            parser.error("download and validation are separate runs")
        download(args.download)
    elif args.source and args.out:
        validate(args.source.resolve(), args.out.resolve(), args.binary.resolve())
    else:
        parser.error("use --download NEW_DIRECTORY or --source DIRECTORY --out NEW_DIRECTORY")


if __name__ == "__main__":
    main()
