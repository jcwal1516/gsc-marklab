#!/usr/bin/env python3
"""Validate cohort Max-T on the open Jackson breast-cancer IMC cohort.

The script downloads the curated Basel SingleCellExperiment from Zenodo, uses
base R only to extract the four required colData columns, reduces cells to
prespecified patient-level fractions, runs Marklab, and checks the result with
an independent NumPy permutation implementation.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import subprocess
import urllib.request
from pathlib import Path

import numpy as np

RECORD_API = "https://zenodo.org/api/records/20621983"
SOURCE_KEY = "sce_basel.rds"
MAXIMUM_SOURCE_BYTES = 300 * 1024 * 1024
ENDPOINTS = tuple(f"metacluster_{cluster:02d}" for cluster in range(1, 14))
GROUP_A = "ER_positive"
GROUP_B = "ER_negative"
MARKLAB_PERMUTATIONS = 19_999
MARKLAB_SEED = 20_261_010
ORACLE_SEED = 20_261_011
ALPHA = 0.05
MAX_EFFECT_ERROR = 1e-12
MAX_STATISTIC_ERROR = 1e-10
# Both adjusted p-values are independent Monte Carlo estimates of the same quantity.
# Their difference is compared with ADJUSTED_P_Z_LIMIT standard errors of a difference of two
# binomial proportions, plus one permutation step on each side for discreteness.
# This replaced a fixed 0.03 absolute limit after review on 2026-10-10: near
# alpha=0.05 that limit admitted errors large enough to change the decision.
ADJUSTED_P_Z_LIMIT = 4.0
HEADERS = {"User-Agent": "Marklab-public-data-validation"}

R_EXTRACT = r"""
args <- commandArgs(trailingOnly = TRUE)
sce <- readRDS(args[[1]])
col_data <- attr(sce, "colData", exact = TRUE)
if (is.null(col_data)) stop("SingleCellExperiment has no serialized colData slot")
columns <- attr(col_data, "listData", exact = TRUE)
if (is.null(columns)) stop("serialized colData has no listData slot")

materialize <- function(value, field) {
    if (is.atomic(value)) return(value)
    values <- attr(value, "values", exact = TRUE)
    lengths <- attr(value, "lengths", exact = TRUE)
    if (!is.null(values) && !is.null(lengths)) return(rep(values, lengths))
    stop(paste("unsupported serialized colData representation for", field))
}

required <- c("patient_id", "tumor_ER_status", "image_name", "cell_metacluster")
missing <- setdiff(required, names(columns))
if (length(missing) != 0) stop(paste("missing colData fields:", paste(missing, collapse = ", ")))
values <- lapply(required, function(field) materialize(columns[[field]], field))
names(values) <- required
lengths <- vapply(values, length, integer(1))
if (length(unique(lengths)) != 1) stop("required colData fields have different lengths")

metadata <- unique(data.frame(
    patient_id = as.character(values$patient_id),
    tumor_ER_status = as.character(values$tumor_ER_status),
    image_name = as.character(values$image_name),
    stringsAsFactors = FALSE
))
write.csv(metadata, args[[2]], row.names = FALSE, na = "")

patient_id <- as.character(values$patient_id)
image_name <- as.character(values$image_name)
cell_metacluster <- suppressWarnings(as.integer(as.character(values$cell_metacluster)))
missing_patient <- is.na(patient_id) | trimws(patient_id) == ""
missing_image <- is.na(image_name) | trimws(image_name) == ""
missing_cluster <- is.na(cell_metacluster)
missing_er <- is.na(values$tumor_ER_status) | trimws(as.character(values$tumor_ER_status)) == ""
write.csv(data.frame(
    raw_cells = length(patient_id), missing_patient_id = sum(missing_patient),
    missing_image_name = sum(missing_image), missing_metacluster = sum(missing_cluster),
    missing_er_status = sum(missing_er)
), args[[4]], row.names = FALSE)
if (any(missing_patient | missing_image)) stop("required patient/image identities are missing")
keep <- !missing_cluster
counts <- aggregate(
    rep.int(1L, sum(keep)),
    by = list(patient_id = patient_id[keep], cell_metacluster = cell_metacluster[keep]),
    FUN = sum
)
names(counts)[[3]] <- "cell_count"
write.csv(counts, args[[3]], row.names = FALSE, na = "")
"""


def request_json(url: str) -> dict:
    request = urllib.request.Request(url, headers=HEADERS)
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)


def write_json(path: Path, value: dict) -> None:
    with path.open("x") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")


def source_metadata() -> dict:
    """Return the reviewed Zenodo file metadata after checking the license."""
    record = request_json(RECORD_API)
    rights = record.get("metadata", {}).get("rights") or []
    right_ids = {str(right.get("id", "")).lower() for right in rights}
    license_id = str(record.get("metadata", {}).get("license", {}).get("id", "")).lower()
    if "cc-by-4.0" not in right_ids | {license_id}:
        raise ValueError("Zenodo record no longer declares CC BY 4.0")
    matches = [entry for entry in record.get("files", []) if entry.get("key") == SOURCE_KEY]
    if len(matches) != 1:
        raise ValueError(f"Zenodo record does not contain exactly one {SOURCE_KEY}")
    entry = matches[0]
    size = int(entry["size"])
    if not 0 < size <= MAXIMUM_SOURCE_BYTES:
        raise ValueError("source exceeds the reviewed bounded-download profile")
    return {
        "record": RECORD_API,
        "doi": record["metadata"]["doi"],
        "title": record["metadata"]["title"],
        "license": "CC BY 4.0",
        "attribution": "Jackson, Hartland W. and colleagues (2020)",
        "filename": SOURCE_KEY,
        "download_url": entry["links"]["self"],
        "expected_bytes": size,
    }


def require_external_directory(directory: Path) -> Path:
    repository = Path(__file__).resolve().parents[2]
    directory = directory.resolve()
    if directory == repository or repository in directory.parents:
        raise ValueError("public cohort data and results must remain outside the checkout")
    return directory


def ensure_source(directory: Path, metadata: dict) -> Path:
    """Download once into a fresh directory, or validate a prior complete source."""
    path = directory / SOURCE_KEY
    if path.exists():
        if path.stat().st_size != metadata["expected_bytes"]:
            raise ValueError("existing source is incomplete; preserve or trash it explicitly")
        return path
    if directory.exists() and any(directory.iterdir()):
        raise ValueError("refusing to add a raw input to a nonempty result directory")
    directory.mkdir(parents=True, exist_ok=True)
    count = 0
    request = urllib.request.Request(metadata["download_url"], headers=HEADERS)
    with urllib.request.urlopen(request, timeout=120) as response, path.open("xb") as stream:
        while chunk := response.read(1024 * 1024):
            count += len(chunk)
            if count > metadata["expected_bytes"]:
                raise ValueError("download exceeds the publisher's declared size")
            stream.write(chunk)
    if count != metadata["expected_bytes"]:
        raise ValueError(f"incomplete download: {count} of {metadata['expected_bytes']} bytes")
    return path


def normalize_er_status(value: str) -> str | None:
    normalized = value.strip().lower()
    if normalized == "positive":
        return GROUP_A
    if normalized == "negative":
        return GROUP_B
    if normalized == "":
        return None
    raise ValueError(f"unexpected nonbinary tumor_ER_status value: {value!r}")


def extract_counts(source: Path, directory: Path, rscript: str) -> tuple[Path, Path]:
    patient_metadata = directory / "patient-image-metadata.csv"
    counts = directory / "patient-metacluster-counts.csv"
    if patient_metadata.exists() or counts.exists():
        raise ValueError("derived extraction outputs already exist; use a fresh result directory")
    subprocess.run(
        [rscript, "--vanilla", "-e", R_EXTRACT, str(source), str(patient_metadata), str(counts),
         str(directory / "source-row-counts.csv")],
        check=True,
    )
    return patient_metadata, counts


def load_patient_metadata(path: Path) -> dict[str, tuple[str, str]]:
    by_patient: dict[str, set[tuple[str, str]]] = {}
    with path.open(newline="") as stream:
        for row in csv.DictReader(stream):
            patient = row["patient_id"].strip()
            status = normalize_er_status(row["tumor_ER_status"])
            image = row["image_name"].strip()
            if patient and status is not None and image:
                by_patient.setdefault(patient, set()).add((status, image))
    conflicts = {patient: values for patient, values in by_patient.items() if len(values) != 1}
    if conflicts:
        raise ValueError(f"patients do not map to exactly one ER status and image: {conflicts}")
    return {patient: next(iter(values)) for patient, values in by_patient.items()}


def prepare_endpoints(
    patient_metadata: Path,
    count_path: Path,
    output_path: Path,
) -> tuple[list[str], np.ndarray, np.ndarray]:
    metadata = load_patient_metadata(patient_metadata)
    counts: dict[str, dict[int, int]] = {patient: {} for patient in metadata}
    totals: dict[str, int] = {patient: 0 for patient in metadata}
    with count_path.open(newline="") as stream:
        for row in csv.DictReader(stream):
            patient = row["patient_id"].strip()
            if patient not in metadata:
                continue
            cluster = int(row["cell_metacluster"])
            count = int(row["cell_count"])
            if not 1 <= cluster <= 27 or count <= 0:
                raise ValueError("metacluster counts must use positive clusters 1 through 27")
            counts[patient][cluster] = counts[patient].get(cluster, 0) + count
            totals[patient] += count
    patients = sorted(patient for patient in metadata if totals[patient] > 0)
    groups = np.asarray([metadata[patient][0] for patient in patients])
    matrix = np.asarray(
        [
            [
                counts[patient].get(cluster, 0) / totals[patient]
                for cluster in range(1, 14)
            ]
            for patient in patients
        ],
        dtype=np.float64,
    )
    group_counts = {group: int(np.sum(groups == group)) for group in (GROUP_A, GROUP_B)}
    if len(patients) != 100:
        raise ValueError(f"expected 100 Basel patients, found {len(patients)}")
    if min(group_counts.values()) < 20:
        raise ValueError(f"each ER group must contain at least 20 patients: {group_counts}")
    if not np.all(np.isfinite(matrix)):
        raise ValueError("patient endpoint matrix contains non-finite values")
    with output_path.open("x", newline="") as stream:
        writer = csv.writer(stream)
        writer.writerow(["patient_id", "group", "endpoint", "value"])
        for patient, group, values in zip(patients, groups, matrix, strict=True):
            for endpoint, value in zip(ENDPOINTS, values, strict=True):
                writer.writerow([patient, group, endpoint, format(float(value), ".17g")])
    return patients, groups, matrix


def welch_statistics(
    matrix: np.ndarray,
    is_group_a: np.ndarray,
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    group_a = matrix[is_group_a]
    group_b = matrix[~is_group_a]
    effects = group_a.mean(axis=0) - group_b.mean(axis=0)
    standard_errors = np.sqrt(
        group_a.var(axis=0, ddof=1) / len(group_a) + group_b.var(axis=0, ddof=1) / len(group_b)
    )
    if np.any(~np.isfinite(standard_errors)) or np.any(standard_errors <= 0):
        raise ValueError("oracle encountered a zero or non-finite Welch standard error")
    return effects, standard_errors, effects / standard_errors


def monte_carlo_p_limit(first: float, second: float, first_draws: int, second_draws: int) -> float:
    """Allowed difference between two independent Monte Carlo p-value estimates."""
    p = (first + second) / 2
    steps = 1 / (first_draws + 1) + 1 / (second_draws + 1)
    return ADJUSTED_P_Z_LIMIT * math.sqrt(p * (1 - p) * steps) + steps


def independent_oracle(
    groups: np.ndarray,
    matrix: np.ndarray,
    permutations: int = MARKLAB_PERMUTATIONS,
) -> dict:
    observed_labels = groups == GROUP_A
    effects, standard_errors, statistics = welch_statistics(matrix, observed_labels)
    rng = np.random.default_rng(ORACLE_SEED)
    exceedances = np.zeros(len(ENDPOINTS), dtype=np.int64)
    maxima = np.empty(permutations, dtype=np.float64)
    for replicate in range(permutations):
        _, _, permuted = welch_statistics(matrix, rng.permutation(observed_labels))
        maximum = float(np.max(np.abs(permuted)))
        maxima[replicate] = maximum
        exceedances += maximum >= np.abs(statistics)
    adjusted = (exceedances + 1) / (permutations + 1)
    rank = math.ceil((1.0 - ALPHA) * (permutations + 1))
    critical_value = float(np.sort(maxima)[min(rank - 1, permutations - 1)])
    return {
        "implementation": "independent_numpy_welch_max_t",
        "seed": ORACLE_SEED,
        "permutations": permutations,
        "critical_value": critical_value,
        "endpoints": [
            {
                "endpoint": endpoint,
                "effect_group_a_minus_group_b": float(effect),
                "standard_error": float(standard_error),
                "studentized_statistic": float(statistic),
                "adjusted_p_value": float(p_value),
            }
            for endpoint, effect, standard_error, statistic, p_value in zip(
                ENDPOINTS, effects, standard_errors, statistics, adjusted, strict=True
            )
        ],
    }


def run_marklab(binary: Path, endpoints: Path, output: Path) -> dict:
    subprocess.run(
        [
            str(binary.resolve()),
            "cohort",
            "max-t",
            "--input",
            str(endpoints),
            "--group-a",
            GROUP_A,
            "--group-b",
            GROUP_B,
            "--permutations",
            str(MARKLAB_PERMUTATIONS),
            "--seed",
            str(MARKLAB_SEED),
            "--alpha",
            str(ALPHA),
            "--out",
            str(output),
        ],
        check=True,
    )
    return json.loads(output.read_text())


def compare(marklab: dict, oracle: dict, groups: np.ndarray) -> dict:
    if marklab.get("format") != "marklab.cohort_max_t" or marklab.get("version") != 1:
        raise ValueError("unexpected Marklab cohort Max-T result identity")
    marklab_rows = marklab["endpoints"]
    oracle_rows = oracle["endpoints"]
    if [row["endpoint"] for row in marklab_rows] != list(ENDPOINTS):
        raise ValueError("Marklab endpoint names or order differ from the prespecified family")
    if [row["endpoint"] for row in oracle_rows] != list(ENDPOINTS):
        raise ValueError("oracle endpoint names or order differ from the prespecified family")
    expected_counts = {
        "group_a_patients": int(np.sum(groups == GROUP_A)),
        "group_b_patients": int(np.sum(groups == GROUP_B)),
    }
    if marklab["groups"] != expected_counts:
        raise ValueError(
            f"Marklab patient counts differ: {marklab['groups']} != {expected_counts}"
        )
    effect_errors = [
        abs(actual["effect_group_a_minus_group_b"] - expected["effect_group_a_minus_group_b"])
        for actual, expected in zip(marklab_rows, oracle_rows, strict=True)
    ]
    statistic_errors = [
        abs(actual["studentized_statistic"] - expected["studentized_statistic"])
        for actual, expected in zip(marklab_rows, oracle_rows, strict=True)
    ]
    p_errors = [
        abs(actual["adjusted_p_value"] - expected["adjusted_p_value"])
        for actual, expected in zip(marklab_rows, oracle_rows, strict=True)
    ]
    p_limits = [
        monte_carlo_p_limit(
            actual["adjusted_p_value"], expected["adjusted_p_value"],
            marklab["permutations"], oracle["permutations"],
        )
        for actual, expected in zip(marklab_rows, oracle_rows, strict=True)
    ]
    passed = (
        max(effect_errors) <= MAX_EFFECT_ERROR
        and max(statistic_errors) <= MAX_STATISTIC_ERROR
        and all(error <= limit for error, limit in zip(p_errors, p_limits, strict=True))
    )
    comparison = {
        "passed": passed,
        "prespecified_acceptance": {
            "maximum_absolute_effect_error": MAX_EFFECT_ERROR,
            "maximum_absolute_studentized_statistic_error": MAX_STATISTIC_ERROR,
            "adjusted_p_value_monte_carlo_z_limit": ADJUSTED_P_Z_LIMIT,
            "adjusted_p_value_limit_amended": "2026-10-10 review; previously a fixed 0.03",
            "finite_nonzero_observed_standard_errors": True,
            "exact_endpoint_names_order_and_patient_counts": True,
        },
        "observed": {
            "maximum_absolute_effect_error": max(effect_errors),
            "maximum_absolute_studentized_statistic_error": max(statistic_errors),
            "maximum_absolute_adjusted_p_value_error": max(p_errors),
            "maximum_adjusted_p_value_error_to_limit_ratio": max(
                error / limit for error, limit in zip(p_errors, p_limits, strict=True)
            ),
        },
    }
    if not passed:
        raise AssertionError(json.dumps(comparison, indent=2))
    return comparison


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path, required=True, help="fresh external result directory"
    )
    parser.add_argument("--marklab", type=Path, required=True, help="existing Marklab binary")
    parser.add_argument("--source", type=Path, help="reuse a complete raw dataset directory read-only")
    parser.add_argument("--rscript", default="Rscript", help="base Rscript executable")
    args = parser.parse_args()

    directory = require_external_directory(args.output)
    metadata = source_metadata()
    if args.source:
        source = require_external_directory(args.source) / SOURCE_KEY
        if not source.is_file() or source.stat().st_size != metadata["expected_bytes"]:
            raise ValueError("reused raw source is absent or has the wrong publisher-declared size")
        directory.mkdir(parents=True, exist_ok=False)
    else:
        source = ensure_source(directory, metadata)
    write_json(directory / "source.json", metadata)
    patient_metadata, counts = extract_counts(source, directory, args.rscript)
    endpoint_path = directory / "endpoints.csv"
    _, groups, matrix = prepare_endpoints(patient_metadata, counts, endpoint_path)
    marklab = run_marklab(args.marklab, endpoint_path, directory / "marklab.json")
    oracle = independent_oracle(groups, matrix)
    write_json(directory / "oracle.json", oracle)
    comparison = compare(marklab, oracle, groups)
    with (directory / "source-row-counts.csv").open(newline="") as stream:
        source_row_counts = {key: int(value) for key, value in next(csv.DictReader(stream)).items()}
    summary = {
        "dataset": "Jackson et al. 2020 Basel IMC cohort",
        "license": metadata["license"],
        "source_row_counts": source_row_counts,
        "patients": len(groups),
        "group_a": {"label": GROUP_A, "patients": int(np.sum(groups == GROUP_A))},
        "group_b": {"label": GROUP_B, "patients": int(np.sum(groups == GROUP_B))},
        "endpoint_family": list(ENDPOINTS),
        "comparison": comparison,
        "marklab_significant_endpoints": [
            row["endpoint"] for row in marklab["endpoints"] if row["adjusted_p_value"] <= ALPHA
        ],
    }
    write_json(directory / "summary.json", summary)
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
