#!/usr/bin/env python3
"""Build the patient-unit CRC spatial phenotype and outcome result bundle."""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import csv
import hashlib
import io
import json
import math
import os
from pathlib import Path
import shutil
import statistics
import sys
import tarfile
import tempfile
from typing import Any, Dict, Iterable, List, Optional, Sequence, Tuple
import xml.etree.ElementTree as ET
import zipfile


sys.path.insert(0, str(Path(__file__).resolve().parent))
from crc_spatial_outcome_statistics import (  # noqa: E402
    benjamini_hochberg,
    fit_cox_breslow,
    patient_label_permutation,
    repeatability_analysis,
)


OBJECTIVE = "CRC-SPATIAL-PHENOTYPE-OUTCOME-01"
SCHEMA_VERSION = "1.0"
SEED = 20_260_828


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def read_csv(path: Path) -> List[Dict[str, str]]:
    if not path.is_file() or path.is_symlink():
        raise ValueError(f"CSV source must be one regular file: {path}")
    with path.open(encoding="utf-8", newline="") as source:
        reader = csv.DictReader(source)
        if reader.fieldnames is None:
            raise ValueError(f"CSV source has no header: {path}")
        return list(reader)


def read_json(path: Path) -> Any:
    if not path.is_file() or path.is_symlink():
        raise ValueError(f"JSON source must be one regular file: {path}")
    return json.loads(path.read_text(encoding="utf-8"))


def read_attribute_matrix(path: Path) -> Dict[str, Dict[str, str]]:
    """Read a patient-column, attribute-row clinical matrix."""
    if not path.is_file() or path.is_symlink():
        raise ValueError(f"clinical matrix must be one regular file: {path}")
    with path.open(encoding="utf-8", newline="") as source:
        rows = list(csv.reader(source, delimiter="\t"))
    if len(rows) < 2 or not rows[0] or rows[0][0] != "attrib_name":
        raise ValueError("clinical matrix must begin with attrib_name")
    patients = rows[0][1:]
    if not patients or any(not patient for patient in patients) or len(set(patients)) != len(patients):
        raise ValueError("clinical matrix patient identities are empty or duplicated")
    result = {patient: {} for patient in patients}
    attributes = set()
    for row in rows[1:]:
        if len(row) != len(patients) + 1 or not row[0] or row[0] in attributes:
            raise ValueError("clinical matrix attributes are malformed or duplicated")
        attributes.add(row[0])
        for patient, value in zip(patients, row[1:]):
            result[patient][row[0]] = value
    return result


def _column_index(reference: str) -> int:
    letters = "".join(character for character in reference if character.isalpha())
    if not letters:
        raise ValueError(f"invalid XLSX cell reference: {reference}")
    result = 0
    for character in letters.upper():
        result = result * 26 + ord(character) - ord("A") + 1
    return result - 1


def read_xlsx_sheet_bytes(raw: bytes, sheet_name: str) -> List[Dict[str, str]]:
    """Read one ordinary XLSX worksheet without an optional Excel dependency."""
    spreadsheet = zipfile.ZipFile(io.BytesIO(raw))
    main = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
    office_rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
    shared: List[str] = []
    if "xl/sharedStrings.xml" in spreadsheet.namelist():
        root = ET.fromstring(spreadsheet.read("xl/sharedStrings.xml"))
        for item in root.findall(f"{{{main}}}si"):
            shared.append("".join(node.text or "" for node in item.iter(f"{{{main}}}t")))
    relationships = ET.fromstring(spreadsheet.read("xl/_rels/workbook.xml.rels"))
    targets = {item.attrib["Id"]: item.attrib["Target"] for item in relationships}
    workbook = ET.fromstring(spreadsheet.read("xl/workbook.xml"))
    target = None
    for sheet in workbook.findall(f".//{{{main}}}sheet"):
        if sheet.attrib.get("name") == sheet_name:
            target = targets[sheet.attrib[f"{{{office_rel}}}id"]]
            break
    if target is None:
        raise ValueError(f"XLSX sheet is absent: {sheet_name}")
    worksheet_path = target.lstrip("/")
    if not worksheet_path.startswith("xl/"):
        worksheet_path = f"xl/{worksheet_path}"
    worksheet = ET.fromstring(spreadsheet.read(worksheet_path))
    materialized: List[Dict[int, str]] = []
    for row in worksheet.findall(f".//{{{main}}}sheetData/{{{main}}}row"):
        values: Dict[int, str] = {}
        for cell in row.findall(f"{{{main}}}c"):
            index = _column_index(cell.attrib.get("r", ""))
            kind = cell.attrib.get("t")
            value_node = cell.find(f"{{{main}}}v")
            value = "" if value_node is None or value_node.text is None else value_node.text
            if kind == "s" and value:
                value = shared[int(value)]
            elif kind == "inlineStr":
                inline = cell.find(f"{{{main}}}is")
                value = "" if inline is None else "".join(
                    node.text or "" for node in inline.iter(f"{{{main}}}t")
                )
            values[index] = value
        materialized.append(values)
    if not materialized:
        raise ValueError(f"XLSX sheet is empty: {sheet_name}")
    source_headers = materialized[0]
    if not source_headers:
        raise ValueError(f"XLSX sheet has no headers: {sheet_name}")
    seen_headers: Counter[str] = Counter()
    headers = {}
    for index, header in source_headers.items():
        if not header:
            continue
        seen_headers[header] += 1
        headers[index] = (
            header if seen_headers[header] == 1 else f"{header}__{seen_headers[header]}"
        )
    rows = []
    for values in materialized[1:]:
        row = {header: values.get(index, "") for index, header in headers.items() if header}
        if any(value != "" for value in row.values()):
            rows.append(row)
    return rows


def read_xlsx_sheet(path: Path, sheet_name: str) -> List[Dict[str, str]]:
    if not path.is_file() or path.is_symlink():
        raise ValueError(f"XLSX source must be one regular file: {path}")
    return read_xlsx_sheet_bytes(path.read_bytes(), sheet_name)


def classify_crc_location(origin: str, project: str) -> str:
    normalized = origin.strip().lower()
    if not normalized or normalized in {"not reported", "#n/a", "[not available]"}:
        return "unavailable"
    if project == "TCGA-READ" or "rectum" in normalized or "rectosigmoid" in normalized:
        return "rectum"
    if any(token in normalized for token in ("cecum", "ascending", "hepatic flexure", "transverse")):
        return "right"
    if any(token in normalized for token in ("splenic flexure", "descending", "sigmoid")):
        return "left"
    return "unavailable"


def _number(value: Any) -> Optional[float]:
    if value is None:
        return None
    text = str(value).strip()
    if not text or text.lower() in {
        "#n/a",
        "na",
        "n/a",
        "nan",
        "none",
        "nd",
        "[not available]",
        "[not applicable]",
        "[unknown]",
    }:
        return None
    try:
        parsed = float(text)
    except ValueError:
        return None
    return parsed if math.isfinite(parsed) else None


def _summary(values: Iterable[float]) -> Dict[str, Any]:
    data = sorted(float(value) for value in values if math.isfinite(float(value)))
    if not data:
        return {"available": False, "count": 0}

    def percentile(probability: float) -> float:
        position = (len(data) - 1) * probability
        lower = int(math.floor(position))
        upper = int(math.ceil(position))
        fraction = position - lower
        return data[lower] * (1.0 - fraction) + data[upper] * fraction

    return {
        "available": True,
        "count": len(data),
        "minimum": data[0],
        "quartile_25": percentile(0.25),
        "median": percentile(0.5),
        "quartile_75": percentile(0.75),
        "maximum": data[-1],
        "positive_count": sum(value > 0.0 for value in data),
        "negative_count": sum(value < 0.0 for value in data),
        "zero_count": sum(value == 0.0 for value in data),
    }


def _write_json(path: Path, value: Any) -> None:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
    path.write_text(encoded + "\n", encoding="utf-8")


def _write_csv(path: Path, fields: Sequence[str], rows: Iterable[Dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8", newline="") as target:
        writer = csv.DictWriter(target, fieldnames=list(fields), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def _unique(rows: Sequence[Dict[str, str]], key: str, label: str) -> Dict[str, Dict[str, str]]:
    result = {}
    for row in rows:
        identity = row.get(key, "")
        if not identity or identity in result:
            raise ValueError(f"{label} has an empty or duplicate {key}")
        result[identity] = row
    return result


def _tcga_cdr(archive_path: Path, member: str) -> Dict[str, Dict[str, str]]:
    with tarfile.open(archive_path, "r:gz") as archive:
        extracted = archive.extractfile(member)
        if extracted is None:
            raise ValueError(f"TCGA-CDR member is absent: {member}")
        rows = read_xlsx_sheet_bytes(extracted.read(), "TCGA-CDR")
    selected = [row for row in rows if row.get("type") in {"COAD", "READ"}]
    return _unique(selected, "bcr_patient_barcode", "TCGA-CDR")


def _tcga_locations(path: Path) -> Dict[str, Dict[str, str]]:
    document = read_json(path)
    hits = document.get("data", {}).get("hits", [])
    result = {}
    for hit in hits:
        case_id = hit.get("case_id") or hit.get("id")
        if not case_id:
            raise ValueError("TCGA GDC location has a missing case identity")
        if case_id in result:
            raise ValueError(f"TCGA GDC location has a duplicate case identity: {case_id}")
        project = hit.get("project", {}).get("project_id", "")
        primary = [
            row
            for row in hit.get("diagnoses", [])
            if str(row.get("classification_of_tumor", "")).lower() == "primary"
        ]
        if not primary and hit.get("diagnoses"):
            primary = hit["diagnoses"][:1]
        origin = primary[0].get("tissue_or_organ_of_origin", "") if primary else ""
        result[case_id] = {
            "raw": str(origin).strip() or "unavailable",
            "group": classify_crc_location(str(origin), str(project)),
        }
    return result


def _long_features(path: Path, cohort: str, lane: str) -> Dict[str, Dict[str, float]]:
    result: Dict[str, Dict[str, float]] = defaultdict(dict)
    for row in read_csv(path):
        if row.get("cohort") != cohort or row.get("lane") != lane:
            continue
        feature = row["feature"]
        patient = row["patient_id"]
        if feature in result[patient]:
            raise ValueError(f"duplicate long feature: {cohort}/{lane}/{patient}/{feature}")
        value = _number(row.get("value"))
        if value is not None:
            result[patient][feature] = value
    return dict(result)


def _repeat_records_from_long(
    path: Path, cohort: str, lane: str, feature_names: Sequence[str]
) -> List[Dict[str, Any]]:
    values: Dict[Tuple[str, str], Dict[str, float]] = defaultdict(dict)
    for row in read_csv(path):
        if row.get("cohort") != cohort or row.get("lane") != lane:
            continue
        if row.get("feature") not in feature_names:
            continue
        identity = (row["patient_id"], row["specimen_id"])
        feature = row["feature"]
        if feature in values[identity]:
            raise ValueError(f"duplicate specimen feature: {identity}/{feature}")
        value = _number(row.get("value"))
        if value is not None:
            values[identity][feature] = value
    return [
        {"patient_id": patient, "specimen_id": specimen, **features}
        for (patient, specimen), features in sorted(values.items())
        if all(name in features for name in feature_names)
    ]


def _safe_cox(
    rows: Sequence[Dict[str, Any]],
    time_name: str,
    event_name: str,
    phenotype: str,
    adjustment_names: Sequence[str],
) -> Dict[str, Any]:
    names = [phenotype, *adjustment_names]
    available = [
        row
        for row in rows
        if row.get(time_name) is not None
        and row.get(event_name) is not None
        and all(row.get(name) is not None for name in names)
    ]
    excluded_nonpositive = sum(row[time_name] <= 0.0 for row in available)
    complete = [row for row in available if row[time_name] > 0.0]
    try:
        result = fit_cox_breslow(
            [row[time_name] for row in complete],
            [int(row[event_name]) for row in complete],
            [[row[name] for name in names] for row in complete],
            names,
        )
        result["endpoint"] = time_name.rsplit("_", 1)[0].upper()
        result["phenotype"] = phenotype
        result["adjustment"] = list(adjustment_names)
        result["complete_case_policy"] = "exact_complete_patient_rows"
        result["excluded_nonpositive_time_count"] = excluded_nonpositive
        return result
    except (ValueError, OverflowError) as error:
        return {
            "available": False,
            "phenotype": phenotype,
            "endpoint": time_name.rsplit("_", 1)[0].upper(),
            "adjustment": list(adjustment_names),
            "complete_patient_count": len(complete),
            "excluded_nonpositive_time_count": excluded_nonpositive,
            "event_count": sum(int(row[event_name]) for row in complete),
            "blocker": f"{type(error).__name__}: {error}",
        }


def _attach_q_values(models: List[Dict[str, Any]]) -> None:
    available = [
        model
        for model in models
        if model.get("effects") and math.isfinite(model["effects"][0]["p_value"])
    ]
    q_values = benjamini_hochberg([model["effects"][0]["p_value"] for model in available])
    for model, q_value in zip(available, q_values):
        model["effects"][0]["benjamini_hochberg_q_value"] = q_value


def _tcga_analysis(args: argparse.Namespace) -> Dict[str, Any]:
    stage = _unique(read_csv(args.tcga_stage), "patient_id", "TCGA stage")
    label_rows = _unique(read_csv(args.tcga_labels), "patient_id", "TCGA labels")
    labels = {patient: row["class_name"] for patient, row in label_rows.items()}
    spatial = _unique(read_csv(args.tcga_spatial), "patient_id", "TCGA spatial")
    microenvironment = _unique(
        read_csv(args.tcga_microenvironment), "patient_id", "TCGA microenvironment"
    )
    cdr = _tcga_cdr(args.tcga_cdr_archive, args.tcga_cdr_member)
    locations = _tcga_locations(args.tcga_gdc)
    m3 = _long_features(args.patient_fingerprints, "TCGA_CRC_CellViT", "m3")
    stable_field_features = ["raw_variogram_intermediate", "raw_variogram_far"]
    field_records = _repeat_records_from_long(
        args.specimen_fingerprints,
        "TCGA_CRC_CellViT",
        "m4_raw_spatial",
        stable_field_features,
    )
    repeatability = repeatability_analysis(
        field_records,
        stable_field_features,
        bootstrap_replicates=args.bootstrap_replicates,
        seed=SEED + 1,
    )
    field_heterogeneity = {
        row["patient_id"]: row["within_patient_distance"]
        for row in repeatability["per_patient"]
    }
    patients = []
    for patient in sorted(set(stage) & set(labels) & set(spatial) & set(microenvironment)):
        stage_row = stage[patient]
        clinical = cdr.get(stage_row.get("submitter_id", ""))
        if clinical is None:
            continue
        stage_text = stage_row.get("stage", "")
        sex = clinical.get("gender", "").upper()
        location = locations.get(
            patient, {"raw": "unavailable", "group": "unavailable"}
        )
        population_sd = [
            value
            for feature, value in m3.get(patient, {}).items()
            if feature.startswith("embedding_raw_cell_population_sd_q")
        ]
        row: Dict[str, Any] = {
            "patient_id": patient,
            "submitter_id": stage_row.get("submitter_id"),
            "site_id": spatial[patient].get("tissue_source_site"),
            "project_id": stage_row.get("project_id"),
            "msi": labels[patient],
            "stage": stage_text,
            "age": _number(clinical.get("age_at_initial_pathologic_diagnosis")),
            "sex": sex if sex in {"MALE", "FEMALE"} else "unavailable",
            "sex_male": 1.0 if sex == "MALE" else 0.0 if sex == "FEMALE" else None,
            "late_stage": 1.0 if stage_text in {"Stage III", "Stage IV"} else 0.0,
            "location": location["raw"],
            "location_group": location["group"],
            "right_location": 1.0 if location["group"] == "right" else 0.0 if location["group"] in {"left", "rectum"} else None,
            "msi_indicator": 1.0 if labels[patient] == "MSI" else 0.0,
            "pfi_event": int(_number(clinical.get("PFI"))) if _number(clinical.get("PFI")) in {0.0, 1.0} else None,
            "pfi_time": _number(clinical.get("PFI.time")),
            "os_event": int(_number(clinical.get("OS"))) if _number(clinical.get("OS")) in {0.0, 1.0} else None,
            "os_time": _number(clinical.get("OS.time")),
            "dfi_event": int(_number(clinical.get("DFI"))) if _number(clinical.get("DFI")) in {0.0, 1.0} else None,
            "dfi_time": _number(clinical.get("DFI.time")),
            "field_embedding_heterogeneity": field_heterogeneity.get(patient),
            "embedding_population_dispersion": statistics.median(population_sd) if population_sd else None,
            "tumor_tumor_organization": _number(spatial[patient].get("short_range_magnitude")),
            "tumor_inflammatory_proximity": _number(
                microenvironment[patient].get("tumor_inflammatory_relative_excess_0_50")
            ),
            "tumor_stromal_proximity": _number(
                microenvironment[patient].get("tumor_connective_relative_excess_0_50")
            ),
        }
        patients.append(row)
    phenotype_names = [
        "field_embedding_heterogeneity",
        "embedding_population_dispersion",
        "tumor_tumor_organization",
        "tumor_inflammatory_proximity",
        "tumor_stromal_proximity",
    ]
    adjustment = ["age", "sex_male", "late_stage", "right_location", "msi_indicator"]
    outcome_models: Dict[str, List[Dict[str, Any]]] = {}
    for endpoint in ("pfi", "os"):
        unadjusted = [
            _safe_cox(patients, f"{endpoint}_time", f"{endpoint}_event", phenotype, [])
            for phenotype in phenotype_names
        ]
        adjusted = [
            _safe_cox(
                patients,
                f"{endpoint}_time",
                f"{endpoint}_event",
                phenotype,
                adjustment,
            )
            for phenotype in phenotype_names
        ]
        _attach_q_values(unadjusted)
        _attach_q_values(adjusted)
        outcome_models[endpoint] = [*unadjusted, *adjusted]
    msi_tests = []
    for index, phenotype in enumerate(phenotype_names):
        records = [
            {"patient_id": row["patient_id"], "group": row["msi"], "value": row[phenotype]}
            for row in patients
            if row.get(phenotype) is not None and row.get("msi") in {"MSI", "MSS"}
        ]
        test = patient_label_permutation(
            records,
            "MSI",
            "MSS",
            permutations=args.permutations,
            seed=SEED + 100 + index,
        )
        test["phenotype"] = phenotype
        msi_tests.append(test)
    q_values = benjamini_hochberg([test["p_value"] for test in msi_tests])
    for test, q_value in zip(msi_tests, q_values):
        test["benjamini_hochberg_q_value"] = q_value
    relationship_summary = {
        "tumor_tumor": {
            "short_range_magnitude": _summary(
                _number(row.get("short_range_magnitude"))
                for row in spatial.values()
                if _number(row.get("short_range_magnitude")) is not None
            ),
            "primary_permutation_p_at_most_0p01_count": sum(
                _number(row.get("primary_0_200um_permutation_p_value")) is not None
                and _number(row.get("primary_0_200um_permutation_p_value")) <= 0.01
                for row in spatial.values()
            ),
        },
        "tumor_inflammatory": {
            band: _summary(
                float(row[f"tumor_inflammatory_relative_excess_{band}"])
                for row in microenvironment.values()
            )
            for band in ("0_25", "25_50", "50_100")
        },
        "tumor_stromal": {
            band: _summary(
                float(row[f"tumor_connective_relative_excess_{band}"])
                for row in microenvironment.values()
            )
            for band in ("0_25", "25_50", "50_100")
        },
    }
    return {
        "patient_rows": patients,
        "repeatability": repeatability,
        "relationships": relationship_summary,
        "msi_tests": msi_tests,
        "outcome_models": outcome_models,
    }


def _roi_embedding_records(path: Path) -> List[Dict[str, Any]]:
    rows = read_json(path)
    records = []
    for position, row in enumerate(rows):
        values = {"primary_0_100": float(row["primary"]["cosine_excess"])}
        for annulus in row["annuli"]:
            if annulus.get("available"):
                name = f"annulus_{int(annulus['lower_um'])}_{int(annulus['upper_um'])}"
                values[name] = float(annulus["cosine_excess"])
        required = ["primary_0_100", "annulus_0_25", "annulus_25_50", "annulus_50_100"]
        if all(name in values for name in required):
            records.append(
                {
                    "patient_id": row["patient_id"],
                    "specimen_id": str(row.get("roi_id") or row.get("slide_id") or position),
                    **values,
                }
            )
    return records


def _cptac_analysis(args: argparse.Namespace) -> Dict[str, Any]:
    signatures = _unique(read_json(args.cptac_signatures), "patient_id", "CPTAC signatures")
    label_rows = _unique(read_csv(args.cptac_labels), "patient_id", "CPTAC labels")
    labels = {patient: row["class_name"] for patient, row in label_rows.items()}
    clinical = read_attribute_matrix(args.cptac_clinical)
    features = ["primary_0_100", "annulus_0_25", "annulus_25_50", "annulus_50_100"]
    repeatability = repeatability_analysis(
        _roi_embedding_records(args.cptac_roi_results),
        features,
        bootstrap_replicates=args.bootstrap_replicates,
        seed=SEED + 2,
    )
    heterogeneity = {
        row["patient_id"]: row["within_patient_distance"]
        for row in repeatability["per_patient"]
    }
    patients = []
    for patient in sorted(signatures):
        clinical_row = clinical.get(patient, {})
        patients.append(
            {
                "patient_id": patient,
                "msi": labels.get(patient),
                "age": _number(clinical_row.get("Age")),
                "age_unit": "undeclared_in_source_matrix",
                "sex": clinical_row.get("Gender", ""),
                "stage": clinical_row.get("Stage", ""),
                "location": clinical_row.get("Subsite", ""),
                "vital_status": clinical_row.get("Vital.Status", ""),
                "tumor_status": clinical_row.get("Tumor.Status", ""),
                "cms": clinical_row.get("Transcriptomic_subtype", ""),
                "tumor_tumor_organization": float(signatures[patient]["median_roi_cosine_excess"]),
                "slide_embedding_heterogeneity": heterogeneity.get(patient),
                "slide_count": int(signatures[patient]["roi_count"]),
            }
        )
    tests = []
    for index, phenotype in enumerate(
        ("tumor_tumor_organization", "slide_embedding_heterogeneity")
    ):
        records = [
            {"patient_id": row["patient_id"], "group": row["msi"], "value": row[phenotype]}
            for row in patients
            if row.get("msi") in {"MSI", "MSS"} and row.get(phenotype) is not None
        ]
        test = patient_label_permutation(
            records,
            "MSI",
            "MSS",
            permutations=args.permutations,
            seed=SEED + 200 + index,
        )
        test["phenotype"] = phenotype
        tests.append(test)
    for test, q_value in zip(tests, benjamini_hochberg([test["p_value"] for test in tests])):
        test["benjamini_hochberg_q_value"] = q_value
    return {
        "patient_rows": patients,
        "repeatability": repeatability,
        "relationships": {
            "tumor_tumor_organization": _summary(
                row["tumor_tumor_organization"] for row in patients
            ),
            "roi_count": sum(row["slide_count"] for row in patients),
        },
        "msi_tests": tests,
    }


def _schurch_analysis(args: argparse.Namespace) -> Dict[str, Any]:
    clinical_rows = read_xlsx_sheet(args.schurch_clinical, "A. Patient_data_TMA_annotations")
    clinical = _unique(
        [
            row
            for row in clinical_rows
            if row.get("Patient", "").isdigit() and 1 <= int(row["Patient"]) <= 35
        ],
        "Patient",
        "Schurch clinical",
    )
    core_rows = read_csv(args.schurch_core_fingerprints)
    core_features = [name for name in core_rows[0] if name.startswith("embedding_")]
    repeatability = repeatability_analysis(
        [
            {
                "patient_id": row["patient_id"],
                "specimen_id": row["core_id"],
                **{name: float(row[name]) for name in core_features},
            }
            for row in core_rows
        ],
        core_features,
        bootstrap_replicates=args.bootstrap_replicates,
        seed=SEED + 3,
    )
    heterogeneity = {
        row["patient_id"]: row["within_patient_distance"]
        for row in repeatability["per_patient"]
    }
    he_signatures = _unique(
        read_json(args.schurch_he_signatures), "patient_id", "Schurch H&E signatures"
    )
    codex_features = _long_features(
        args.patient_fingerprints, "Schurch_CODEX_2020", "orthogonal_neighborhood"
    )
    patients = []
    for patient in sorted(clinical, key=int):
        row = clinical[patient]
        os_event = _number(row.get("OS_Censor"))
        dfs_event = _number(row.get("DFS_Censor"))
        post = _number(row.get("PostOP_Therapy"))
        patients.append(
            {
                "patient_id": patient,
                "msi": row.get("MSI_IHC") if row.get("MSI_IHC") in {"MSI", "MSS"} else "nd",
                "age": _number(row.get("Age")),
                "sex": row.get("Sex", ""),
                "stage": "IV" if str(row.get("cp_TNM_Simple")) == "4" else "III",
                "location": row.get("Simple_Tumor_Location", ""),
                "postoperative_therapy": int(post) if post in {0.0, 1.0} else None,
                "os_time": _number(row.get("OS")),
                "os_event": int(os_event) if os_event in {0.0, 1.0} else None,
                "dfs_time": _number(row.get("DFS")),
                "dfs_event": int(dfs_event) if dfs_event in {0.0, 1.0} else None,
                "he_tumor_tumor_organization": float(he_signatures[patient]["median_roi_cosine_excess"]),
                "codex_core_heterogeneity": heterogeneity.get(patient),
                "codex_immune_infiltrated_stroma": codex_features.get(patient, {}).get(
                    "embedding_microenvironment_immune_infiltrated_stroma_fraction"
                ),
                "codex_tumor_boundary": codex_features.get(patient, {}).get(
                    "embedding_microenvironment_tumor_boundary_fraction"
                ),
                "mmr_pattern": "/".join(
                    row.get(name, "nd") or "nd" for name in ("MLH1", "PMS2", "MSH6", "MSH2")
                ),
            }
        )
    phenotypes = [
        "he_tumor_tumor_organization",
        "codex_core_heterogeneity",
        "codex_immune_infiltrated_stroma",
        "codex_tumor_boundary",
    ]
    outcome_models = {}
    for endpoint in ("os", "dfs"):
        models = [
            _safe_cox(patients, f"{endpoint}_time", f"{endpoint}_event", phenotype, [])
            for phenotype in phenotypes
        ]
        _attach_q_values(models)
        outcome_models[endpoint] = models
    msi_tests = []
    for index, phenotype in enumerate(phenotypes):
        records = [
            {"patient_id": row["patient_id"], "group": row["msi"], "value": row[phenotype]}
            for row in patients
            if row["msi"] in {"MSI", "MSS"} and row.get(phenotype) is not None
        ]
        test = patient_label_permutation(
            records,
            "MSI",
            "MSS",
            permutations=args.permutations,
            seed=SEED + 300 + index,
        )
        test["phenotype"] = phenotype
        msi_tests.append(test)
    for test, q_value in zip(
        msi_tests, benjamini_hochberg([test["p_value"] for test in msi_tests])
    ):
        test["benjamini_hochberg_q_value"] = q_value
    return {
        "patient_rows": patients,
        "repeatability": repeatability,
        "relationships": {
            "he_tumor_tumor_organization": _summary(
                row["he_tumor_tumor_organization"] for row in patients
            ),
            "mmr_protein_loss_patterns": dict(sorted(Counter(row["mmr_pattern"] for row in patients).items())),
        },
        "msi_tests": msi_tests,
        "outcome_models": outcome_models,
    }


def _stanford_class(cell_type: str) -> str:
    if cell_type in {"Epithelial/Tumor", "P53+ Tumor", "Proliferative Tumor"}:
        return "tumor"
    if cell_type in {
        "B cell",
        "CD4 T cell",
        "CD8 T cell",
        "Macrophages",
        "NK-cells",
        "Proliferative Immune",
    }:
        return "immune"
    if cell_type == "Stromal":
        return "stroma"
    return "other"


def _roi_ecology(points: Sequence[Tuple[float, float, float, str]]) -> Dict[str, float]:
    if len(points) < 20:
        raise ValueError("Stanford ROI has fewer than 20 cells")
    equivalent_diameters = [2.0 * math.sqrt(area / math.pi) for _, _, area, _ in points]
    radius = 4.0 * statistics.median(equivalent_diameters)
    if not math.isfinite(radius) or radius <= 0.0:
        raise ValueError("Stanford ROI adaptive neighborhood radius is invalid")
    grid: Dict[Tuple[int, int], List[int]] = defaultdict(list)
    for index, (x, y, _, _) in enumerate(points):
        grid[(math.floor(x / radius), math.floor(y / radius))].append(index)
    edges = 0
    observed = Counter()
    for left, (x, y, _, left_class) in enumerate(points):
        gx, gy = math.floor(x / radius), math.floor(y / radius)
        for dx in (-1, 0, 1):
            for dy in (-1, 0, 1):
                for right in grid.get((gx + dx, gy + dy), []):
                    if right <= left:
                        continue
                    rx, ry, _, right_class = points[right]
                    if (x - rx) ** 2 + (y - ry) ** 2 <= radius * radius:
                        edges += 1
                        observed[tuple(sorted((left_class, right_class)))] += 1
    if edges == 0:
        raise ValueError("Stanford ROI adaptive graph has no edges")
    counts = Counter(point[3] for point in points)
    count = len(points)

    def excess(left: str, right: str) -> Optional[float]:
        if left == right:
            probability = counts[left] * (counts[left] - 1) / (count * (count - 1))
        else:
            probability = 2.0 * counts[left] * counts[right] / (count * (count - 1))
        expected = edges * probability
        if expected <= 0.0:
            return None
        return observed[tuple(sorted((left, right)))] / expected - 1.0

    return {
        "tumor_fraction": counts["tumor"] / count,
        "immune_fraction": counts["immune"] / count,
        "stroma_fraction": counts["stroma"] / count,
        "other_fraction": counts["other"] / count,
        "tumor_tumor_proximity": excess("tumor", "tumor"),
        "tumor_immune_proximity": excess("tumor", "immune"),
        "tumor_stromal_proximity": excess("tumor", "stroma"),
        "adaptive_radius_source_units": radius,
        "cell_count": float(count),
        "edge_count": float(edges),
    }


def _stanford_roi_records(path: Path) -> List[Dict[str, Any]]:
    grouped: Dict[Tuple[str, str], List[Tuple[float, float, float, str]]] = defaultdict(list)
    for row in read_csv(path):
        if row.get("batch") != "Cancer":
            continue
        patient = row["alt_identifier"]
        roi = f"{row['batch']}:{row['Metadata_acid']}:{row['ImageNumber']}"
        x = _number(row.get("AreaShape_Center_X"))
        y = _number(row.get("AreaShape_Center_Y"))
        area = _number(row.get("AreaShape_Area"))
        if x is None or y is None or area is None or area <= 0.0:
            raise ValueError("Stanford cell has invalid coordinates or area")
        grouped[(patient, roi)].append((x, y, area, _stanford_class(row["cell_type"])))
    records = []
    for (patient, roi), points in sorted(grouped.items()):
        records.append({"patient_id": patient, "specimen_id": roi, **_roi_ecology(points)})
    return records


def _stanford_analysis(args: argparse.Namespace) -> Dict[str, Any]:
    clinical = _unique(read_csv(args.stanford_clinical), "alt_identifier", "Stanford clinical")
    roi_records = _stanford_roi_records(args.stanford_cells)
    features = [
        "tumor_fraction",
        "immune_fraction",
        "stroma_fraction",
        "tumor_tumor_proximity",
        "tumor_immune_proximity",
        "tumor_stromal_proximity",
    ]
    complete_repeat_records = [
        row for row in roi_records if all(row.get(name) is not None for name in features)
    ]
    repeatability = repeatability_analysis(
        complete_repeat_records,
        features,
        bootstrap_replicates=args.bootstrap_replicates,
        seed=SEED + 4,
    )
    heterogeneity = {
        row["patient_id"]: row["within_patient_distance"]
        for row in repeatability["per_patient"]
    }
    by_patient: Dict[str, List[Dict[str, Any]]] = defaultdict(list)
    for row in roi_records:
        by_patient[row["patient_id"]].append(row)
    patients = []
    for patient in sorted(clinical):
        if patient not in by_patient:
            continue
        row = clinical[patient]
        recurrence_raw = row.get("Reccurrance", "").strip().lower()
        recurrence = "recur" if recurrence_raw == "recur" else "no_recur" if recurrence_raw == "no recur" else "unknown"
        msi_raw = row.get("MSI_any", "").strip().lower()
        msi = "MSI" if msi_raw == "true" else "MSS" if msi_raw == "false" else "unknown"
        rois = by_patient[patient]

        def patient_median(name: str) -> Optional[float]:
            values = [r[name] for r in rois if r.get(name) is not None]
            return statistics.median(values) if values else None

        patients.append(
            {
                "patient_id": patient,
                "recurrence": recurrence,
                "msi": msi,
                "age": _number(row.get("DxAge")),
                "sex": row.get("Gender", ""),
                "stage": row.get("SummaryStage", ""),
                "location": row.get("Site2", ""),
                "site": row.get("BXFacilityID", ""),
                "treatment": row.get("Treatment", ""),
                "roi_count": len(rois),
                "roi_ecology_heterogeneity": heterogeneity.get(patient),
                "tumor_tumor_proximity": patient_median("tumor_tumor_proximity"),
                "tumor_immune_proximity": patient_median("tumor_immune_proximity"),
                "tumor_stromal_proximity": patient_median("tumor_stromal_proximity"),
            }
        )
    phenotypes = [
        "roi_ecology_heterogeneity",
        "tumor_tumor_proximity",
        "tumor_immune_proximity",
        "tumor_stromal_proximity",
    ]

    def tests_for(label: str, group_a: str, group_b: str, seed_offset: int) -> List[Dict[str, Any]]:
        tests = []
        for index, phenotype in enumerate(phenotypes):
            records = [
                {"patient_id": row["patient_id"], "group": row[label], "value": row[phenotype]}
                for row in patients
                if row.get(label) in {group_a, group_b} and row.get(phenotype) is not None
            ]
            test = patient_label_permutation(
                records,
                group_a,
                group_b,
                permutations=args.permutations,
                seed=SEED + seed_offset + index,
            )
            test["phenotype"] = phenotype
            tests.append(test)
        for test, q_value in zip(tests, benjamini_hochberg([test["p_value"] for test in tests])):
            test["benjamini_hochberg_q_value"] = q_value
        return tests

    return {
        "patient_rows": patients,
        "repeatability": repeatability,
        "relationships": {
            phenotype: _summary(row[phenotype] for row in patients if row.get(phenotype) is not None)
            for phenotype in phenotypes[1:]
        },
        "recurrence_tests": tests_for("recurrence", "recur", "no_recur", 400),
        "msi_tests": tests_for("msi", "MSI", "MSS", 500),
        "survival_blocker": "DaysSurvival has no event/censor indicator and was not analyzed as time-to-event",
    }


def _manifest_rows(results: Dict[str, Dict[str, Any]]) -> List[Dict[str, Any]]:
    rows = []
    for cohort, result in results.items():
        for row in result["patient_rows"]:
            rows.append(
                {
                    "cohort": cohort,
                    "patient_id": row["patient_id"],
                    "patient_is_population_unit": "true",
                    "repeated_measure_count": row.get("slide_count") or row.get("roi_count") or "",
                    "msi_available": str(row.get("msi") in {"MSI", "MSS"}).lower(),
                    "survival_available": str(
                        row.get("pfi_time") is not None
                        or row.get("os_time") is not None
                        or row.get("dfs_time") is not None
                    ).lower(),
                    "recurrence_available": str(row.get("recurrence") in {"recur", "no_recur"}).lower(),
                    "stage_available": str(bool(row.get("stage"))).lower(),
                    "location_available": str(row.get("location") not in {None, "", "unavailable"}).lower(),
                    "age_available": str(row.get("age") is not None).lower(),
                    "sex_available": str(row.get("sex") not in {None, "", "unavailable"}).lower(),
                    "treatment_available": str(row.get("treatment") not in {None, "", "unknown"} or row.get("postoperative_therapy") is not None).lower(),
                }
            )
    return rows


def _event_counts(results: Dict[str, Dict[str, Any]]) -> Dict[str, Any]:
    tcga = results["TCGA_CRC"]["patient_rows"]
    schurch = results["Schurch_CRC"]["patient_rows"]
    stanford = results["Stanford_Intermountain_CRC"]["patient_rows"]
    return {
        "TCGA_CRC": {
            endpoint: {
                "patients": sum(row.get(f"{endpoint}_time") is not None and row.get(f"{endpoint}_event") is not None for row in tcga),
                "events": sum(row.get(f"{endpoint}_event") == 1 for row in tcga),
                "censored": sum(row.get(f"{endpoint}_event") == 0 for row in tcga),
            }
            for endpoint in ("pfi", "os", "dfi")
        },
        "Schurch_CRC": {
            endpoint: {
                "patients": sum(row.get(f"{endpoint}_time") is not None and row.get(f"{endpoint}_event") is not None for row in schurch),
                "events": sum(row.get(f"{endpoint}_event") == 1 for row in schurch),
                "censored": sum(row.get(f"{endpoint}_event") == 0 for row in schurch),
            }
            for endpoint in ("os", "dfs")
        },
        "Stanford_Intermountain_CRC": {
            "recurrence_known": sum(row.get("recurrence") in {"recur", "no_recur"} for row in stanford),
            "recur": sum(row.get("recurrence") == "recur" for row in stanford),
            "no_recur": sum(row.get("recurrence") == "no_recur" for row in stanford),
            "unknown": sum(row.get("recurrence") == "unknown" for row in stanford),
        },
    }


def build(args: argparse.Namespace) -> None:
    output = args.out.resolve()
    if output.exists() or output.is_symlink():
        raise ValueError(f"output already exists: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix=f".{output.name}.part-", dir=output.parent))
    try:
        results = {
            "TCGA_CRC": _tcga_analysis(args),
            "CPTAC_CRC": _cptac_analysis(args),
            "Schurch_CRC": _schurch_analysis(args),
            "Stanford_Intermountain_CRC": _stanford_analysis(args),
        }
        manifest_rows = _manifest_rows(results)
        _write_csv(
            staging / "patient_lane_manifest.csv",
            [
                "cohort",
                "patient_id",
                "patient_is_population_unit",
                "repeated_measure_count",
                "msi_available",
                "survival_available",
                "recurrence_available",
                "stage_available",
                "location_available",
                "age_available",
                "sex_available",
                "treatment_available",
            ],
            manifest_rows,
        )
        for cohort, result in results.items():
            _write_json(staging / f"{cohort.lower()}_analysis.json", result)
        event_counts = _event_counts(results)
        _write_json(staging / "event_counts.json", event_counts)
        design = {
            "schema_name": "marklab_crc_spatial_phenotype_outcome_design",
            "schema_version": SCHEMA_VERSION,
            "objective": OBJECTIVE,
            "population_unit": "patient",
            "lower_level_units": "cells, fields, slides, ROIs, and cores are repeated measurements only",
            "feature_construction": "outcome_blind_preexisting_hash_sealed_spatial_artifacts",
            "aims": [
                {
                    "aim": 1,
                    "question": "How much spatial and phenotypic heterogeneity exists within one tumor?",
                    "estimands": "robust-scaled field/slide/core/ROI within-patient distances and patient-first bootstrap intervals",
                },
                {
                    "aim": 2,
                    "question": "Are tumor phenotypes locally related, and how do tumor cells relate to immune and stromal compartments?",
                    "estimands": "CellViT embedding cosine excess and composition-conditioned tumor-immune/tumor-stroma proximity excess",
                },
                {
                    "aim": 3,
                    "question": "Are spatial phenotypes reproducible across tumors or patient-unique?",
                    "estimands": "within-versus-between distance and leave-one-specimen-out patient retrieval with whole-patient bootstrap",
                },
                {
                    "aim": 4,
                    "question": "Do frozen phenotypes associate with MSI, recurrence, PFI, OS, or DFS?",
                    "estimands": "whole-patient label permutations and Breslow-tie Cox models per phenotype standard deviation",
                },
            ],
            "primary_outcome": "TCGA progression-free interval",
            "secondary_outcomes": ["TCGA overall survival", "Schurch overall survival", "Schurch DFS", "Stanford recurrence"],
            "multiplicity": "Benjamini-Hochberg within each cohort-endpoint phenotype family",
            "cox_adjustment": ["age", "sex", "late stage", "right versus left/rectal location", "MSI"],
            "claim_ceiling": "retrospective exploratory association; no causal, clinical-utility, clone, or treatment-effect claim",
        }
        _write_json(staging / "analysis_design.json", design)
        blockers = {
            "CPTAC_CRC": [
                "vital status has no follow-up/censoring time; no Cox or time-to-event analysis",
                "tumor status is not recurrence",
                "no MMR protein-loss, BRAF, CIMP, Lynch, or treatment exposure lane",
            ],
            "TCGA_CRC": [
                "one diagnostic slide per patient; sampled fields measure within-slide heterogeneity, not multiblock whole-tumor coverage",
                "treatment_outcome_first_course is response, not a treatment exposure",
                "no complete MMR protein-loss, CIMP, CMS, or Lynch lane",
            ],
            "Schurch_CRC": [
                "DFS event meaning is not explicitly recurrence-specific in the local table",
                "BRAF, CIMP, CMS, and Lynch annotations are unavailable",
                "H&E CellViT and CODEX features are conceptually related but not one raw feature space",
            ],
            "Stanford_Intermountain_CRC": [
                "DaysSurvival lacks an event/censor indicator and is not a survival endpoint",
                "IMC coordinate unit is undeclared; adaptive neighborhoods remain in source-coordinate units",
                "MMR/BRAF testing is sparse and CIMP/CMS/Lynch are unavailable",
            ],
            "patch_lane": "independent raw patch-vector tensors are unavailable in TCGA and CPTAC CellViT artifacts; patch analyses remain cell-token aggregates only",
            "Heiser_CRC": "only nine CRC patients have downloaded spatial assets and vital status has no follow-up time; ancillary only, no inferential lane",
        }
        _write_json(staging / "blockers.json", blockers)
        source_paths = [
            args.tcga_spatial,
            args.tcga_microenvironment,
            args.tcga_stage,
            args.tcga_labels,
            args.tcga_gdc,
            args.tcga_cdr_archive,
            args.patient_fingerprints,
            args.specimen_fingerprints,
            args.cptac_signatures,
            args.cptac_roi_results,
            args.cptac_labels,
            args.cptac_clinical,
            args.schurch_clinical,
            args.schurch_core_fingerprints,
            args.schurch_he_signatures,
            args.stanford_clinical,
            args.stanford_cells,
        ]
        provenance = {
            "schema_name": "marklab_crc_spatial_phenotype_outcome_provenance",
            "schema_version": SCHEMA_VERSION,
            "objective": OBJECTIVE,
            "adapter_sha256": sha256(Path(__file__).resolve()),
            "statistics_sha256": sha256(Path(__file__).with_name("crc_spatial_outcome_statistics.py")),
            "source_sha256": {str(path.resolve()): sha256(path.resolve()) for path in source_paths},
            "tcga_cdr_member": args.tcga_cdr_member,
            "seed": SEED,
            "bootstrap_replicates": args.bootstrap_replicates,
            "permutations": args.permutations,
        }
        _write_json(staging / "provenance.json", provenance)
        provenance_directory = staging / "provenance"
        provenance_directory.mkdir()
        shutil.copy2(Path(__file__).resolve(), provenance_directory / Path(__file__).name)
        statistics_path = Path(__file__).with_name("crc_spatial_outcome_statistics.py")
        shutil.copy2(statistics_path, provenance_directory / statistics_path.name)
        summary = {
            "schema_name": "marklab_crc_spatial_phenotype_outcome_summary",
            "schema_version": SCHEMA_VERSION,
            "objective": OBJECTIVE,
            "population_unit": "patient",
            "patient_manifest_rows": len(manifest_rows),
            "event_counts": event_counts,
            "repeatability": {
                cohort: {
                    key: result["repeatability"][key]
                    for key in (
                        "patient_count",
                        "repeated_patient_count",
                        "specimen_count",
                        "median_within_patient_distance",
                        "median_between_patient_distance",
                        "median_between_minus_within_distance",
                        "leave_one_specimen_out_patient_retrieval",
                        "bootstrap",
                    )
                }
                for cohort, result in results.items()
            },
            "relationship_summaries": {
                cohort: result["relationships"] for cohort, result in results.items()
            },
            "interpretation": "Outcome-blind spatial phenotypes are evaluated without treating cells, fields, slides, ROIs, or cores as independent patients.",
            "claim_status": "retrospective_exploratory_patient_level_association",
        }
        _write_json(staging / "scientific_summary.json", summary)
        files = sorted(path for path in staging.rglob("*") if path.is_file())
        _write_json(
            staging / "manifest.json",
            {
                "schema_name": "marklab_crc_spatial_phenotype_outcome_manifest",
                "schema_version": SCHEMA_VERSION,
                "objective": OBJECTIVE,
                "files_sha256": {
                    path.relative_to(staging).as_posix(): sha256(path) for path in files
                },
            },
        )
        os.replace(staging, output)
    except BaseException:
        shutil.rmtree(staging, ignore_errors=True)
        raise


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--tcga-spatial", required=True, type=Path)
    result.add_argument("--tcga-microenvironment", required=True, type=Path)
    result.add_argument("--tcga-stage", required=True, type=Path)
    result.add_argument("--tcga-labels", required=True, type=Path)
    result.add_argument("--tcga-gdc", required=True, type=Path)
    result.add_argument("--tcga-cdr-archive", required=True, type=Path)
    result.add_argument("--tcga-cdr-member", required=True)
    result.add_argument("--patient-fingerprints", required=True, type=Path)
    result.add_argument("--specimen-fingerprints", required=True, type=Path)
    result.add_argument("--cptac-signatures", required=True, type=Path)
    result.add_argument("--cptac-roi-results", required=True, type=Path)
    result.add_argument("--cptac-labels", required=True, type=Path)
    result.add_argument("--cptac-clinical", required=True, type=Path)
    result.add_argument("--schurch-clinical", required=True, type=Path)
    result.add_argument("--schurch-core-fingerprints", required=True, type=Path)
    result.add_argument("--schurch-he-signatures", required=True, type=Path)
    result.add_argument("--stanford-clinical", required=True, type=Path)
    result.add_argument("--stanford-cells", required=True, type=Path)
    result.add_argument("--bootstrap-replicates", type=int, default=1000)
    result.add_argument("--permutations", type=int, default=9999)
    result.add_argument("--out", required=True, type=Path)
    return result


if __name__ == "__main__":
    try:
        build(parser().parse_args())
    except Exception as error:
        print(
            f"CRC spatial phenotype outcome analysis failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
