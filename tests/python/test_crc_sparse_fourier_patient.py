import csv
import importlib.util
import json
import os
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_crc_sparse_fourier_patient.py"
)


def load_module():
    spec = importlib.util.spec_from_file_location("crc_sparse_fourier_patient", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


VARIANTS = ("baseline", "subsample", "jitter", "radius_45", "radius_55")


def write_csv(path, fieldnames, rows):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=fieldnames, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n")


def graph_request(pattern_index, variant_index):
    nodes = []
    for component in range(2):
        for within in range(6):
            nodes.append(
                {
                    "id": f"cell-{pattern_index}-{component}-{within}",
                    "coordinates_um": [component * 100.0 + within, variant_index * 0.01],
                    "signal": float((pattern_index + within) % 3 == 0),
                }
            )
    return {
        "nodes": nodes,
        "radius_um": 1.1,
        "maximum_candidate_pairs": 66,
        "maximum_edges": 20,
    }


def build_prepared(root):
    rows = []
    for patient_index, patient in enumerate(("P1", "P2", "P3", "P4")):
        group = "MSI" if patient_index < 2 else "MSS"
        for slide in range(2):
            pattern_index = patient_index * 2 + slide
            pattern = f"{patient}-S{slide + 1}"
            row = {
                "pattern_id": pattern,
                "patient_id": patient,
                "group": group,
                "pattern_count_for_patient": "2",
                "cell_count": "12",
            }
            for variant_index, variant in enumerate(VARIANTS):
                path = f"source/{pattern}/{variant}.json"
                row[f"graph_{variant}_request"] = path
                write_json(root / path, graph_request(pattern_index, variant_index))
            rows.append(row)
    write_csv(root / "manifest.csv", list(rows[0]), rows)


def result_document(pattern_index, variant_index):
    centered = 80.0
    low_fraction = 0.12 + 0.025 * pattern_index + 0.002 * variant_index
    nonzero_total = centered * low_fraction
    mode_energies = [
        {
            "mode_index": index,
            "component_index": index,
            "component_node_count": 6,
            "eigenvalue": 0.0,
            "coefficient": 3.0,
            "energy": 9.0,
            "component_zero_mode": True,
        }
        for index in range(2)
    ]
    for offset in range(8):
        energy = nonzero_total / 8.0
        mode_energies.append(
            {
                "mode_index": offset + 2,
                "component_index": offset % 2,
                "component_node_count": 6,
                "eigenvalue": 0.1 + 0.01 * offset + 0.001 * pattern_index,
                "coefficient": energy**0.5,
                "energy": energy,
                "component_zero_mode": False,
            }
        )
    return {
        "format": "marklab.graph_sparse_radius_fourier_energy",
        "version": 1,
        "node_count": 12,
        "edge_count": 10,
        "isolated_node_count": 0,
        "component_count": 2,
        "component_sizes": [6, 6],
        "returned_mode_count": 10,
        "total_signal_energy": 98.0,
        "component_zero_energy": 18.0,
        "centered_signal_energy": centered,
        "nonzero_low_frequency_energy": nonzero_total,
        "captured_centered_energy_fraction": low_fraction,
        "total_signal_status": "positive",
        "mode_energies": mode_energies,
    }


class SparseFourierPatientTests(unittest.TestCase):
    def test_prepare_preserves_patient_nesting_and_fixed_nonzero_mode_count(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            first = root / "first"
            second = root / "second"
            build_prepared(source)
            manifest = module.prepare_requests(source, first, 8, 10_000)
            replay = module.prepare_requests(source, second, 8, 10_000)
            self.assertEqual(len(manifest), 8)
            self.assertEqual({row["patient_id"] for row in manifest}, {"P1", "P2", "P3", "P4"})
            self.assertTrue(all(row["baseline_component_count"] == 2 for row in manifest))
            self.assertTrue(all(row["baseline_mode_count"] == 10 for row in manifest))
            self.assertEqual((first / "manifest.csv").read_bytes(), (second / "manifest.csv").read_bytes())
            self.assertEqual(manifest, replay)

    def test_fourier_features_use_exactly_eight_nonzero_modes(self):
        module = load_module()
        features = module.fourier_features(result_document(3, 0), 2, 8)
        self.assertAlmostEqual(features["component_fraction"], 2 / 12)
        self.assertAlmostEqual(features["component_zero_energy_fraction"], 18 / 98)
        self.assertAlmostEqual(features["low8_centered_energy_fraction"], 0.195)
        self.assertGreater(features["low8_energy_weighted_eigenvalue"], 0.0)

    def test_execute_runs_bounded_project_misses_then_byte_identical_hits(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            prepared = root / "prepared"
            execution = root / "execution"
            fake = root / "fake-marklab"
            build_prepared(source)
            module.prepare_requests(source, prepared, 8, 10_000)
            fake.write_text(
                """#!/usr/bin/env python3
import json, pathlib, sys
args = sys.argv[1:]
project = pathlib.Path(args[args.index('--project') + 1])
source = pathlib.Path(args[args.index('--input') + 1])
output = pathlib.Path(args[args.index('--out') + 1])
project.mkdir(parents=True, exist_ok=True)
marker = project / 'execution'
status = 'hit' if marker.exists() else 'miss'
if status == 'miss':
    marker.write_text('one\\n')
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps(json.loads(source.read_text()), sort_keys=True) + '\\n')
print(f'project sparse-radius-fourier-energy cache_status={status}', file=sys.stderr)
""",
                encoding="utf-8",
            )
            os.chmod(fake, 0o755)
            misses = module.execute_requests(prepared, fake, execution, 2, 10, False)
            hits = module.execute_requests(prepared, fake, execution, 2, 10, True)
            self.assertEqual(misses["cache_status_counts"], {"miss": 40})
            self.assertEqual(hits["cache_status_counts"], {"hit": 40})
            self.assertTrue(hits["all_replay_bytes_equal"])

    def test_summary_keeps_patient_as_population_unit(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            prepared = root / "prepared"
            execution = root / "execution"
            output = root / "summary"
            baseline = root / "baseline.csv"
            build_prepared(source)
            manifest = module.prepare_requests(source, prepared, 8, 10_000)
            baseline_rows = []
            patients = sorted({row["patient_id"] for row in manifest})
            for patient_index, patient in enumerate(patients):
                group = "MSI" if patient_index < 2 else "MSS"
                for feature, value in (("a", patient_index), ("b", patient_index**2)):
                    baseline_rows.append(
                        {"patient_id": patient, "group": group, "feature": feature, "value": value}
                    )
            write_csv(baseline, ["patient_id", "group", "feature", "value"], baseline_rows)
            for pattern_index, row in enumerate(manifest):
                for variant_index, variant in enumerate(VARIANTS):
                    write_json(
                        execution / "results" / variant / f"{row['pattern_id']}.json",
                        result_document(pattern_index, variant_index),
                    )
            summary = module.summarize(prepared, execution, baseline, output, 20260829)
            self.assertEqual(summary["population_unit"], "patient")
            self.assertEqual(summary["patient_count"], 4)
            self.assertEqual(summary["pattern_count"], 8)
            self.assertTrue(summary["leakage_checks"]["patient_held_out"])
            self.assertIn("no_acquisition_site", summary["leakage_checks"]["site_held_out"])
            self.assertTrue((output / "patient_fingerprints.csv").is_file())
            self.assertTrue((output / "manifest.json").is_file())


if __name__ == "__main__":
    unittest.main()
