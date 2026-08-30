import importlib.util
import csv
import json
import os
from pathlib import Path
import tempfile
import unittest


MODULE_PATH = (
    Path(__file__).resolve().parents[2]
    / "workers/python/marklab_gastric_cellvit_interim.py"
)


def load_module():
    spec = importlib.util.spec_from_file_location("gastric_cellvit_interim", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class GastricCellvitInterimTest(unittest.TestCase):
    def test_full_tissue_rows_form_four_deterministic_bounded_fields(self):
        module = load_module()
        rows = []
        vectors = []
        for index, (x, y, cell_type) in enumerate(
            [
                (0, 0, 1),
                (1, 0, 2),
                (100, 0, 1),
                (101, 0, 3),
                (0, 100, 2),
                (1, 100, 1),
                (100, 100, 3),
                (101, 100, 1),
                (2, 1, 1),
                (102, 1, 2),
                (2, 101, 3),
                (102, 101, 1),
            ]
        ):
            rows.append({"index": index, "x_um": x, "y_um": y, "type_code": cell_type})
            vectors.append([float(index), float(index + 1)])
        patch_centres = [(0.0, 0.0), (100.0, 0.0), (0.0, 100.0), (100.0, 100.0)]

        first = module.prepare_slide(
            "slide-a", rows, vectors, patch_centres, maximum_field_cells=2, seed=731
        )
        second = module.prepare_slide(
            "slide-a", rows, vectors, patch_centres, maximum_field_cells=2, seed=731
        )

        self.assertEqual(first, second)
        self.assertEqual(first["cell_count"], 12)
        self.assertEqual(first["embedding_width"], 2)
        self.assertEqual(first["embedding_mean"], [5.5, 6.5])
        self.assertEqual(len(first["fields"]), 4)
        self.assertTrue(all(field["source_cell_count"] == 3 for field in first["fields"]))
        self.assertTrue(all(field["sampled_cell_count"] == 2 for field in first["fields"]))
        self.assertEqual(
            sum(field["source_cell_count"] for field in first["fields"]), 12
        )

    def test_requests_keep_physical_scales_and_explicit_work_bounds(self):
        module = load_module()
        cells = [
            {
                "id": f"slide:field:{index:04d}",
                "x_um": float(index * 10),
                "y_um": float((index % 3) * 7),
                "type_code": 1 if index % 2 == 0 else 2,
            }
            for index in range(12)
        ]

        requests = module.field_requests(cells, seed=731)

        self.assertEqual(set(requests["graph"]), {"baseline", "subsample", "jitter", "radius_45", "radius_55"})
        self.assertEqual(requests["graph"]["baseline"]["radius_um"], 50.0)
        self.assertEqual(requests["graph"]["radius_45"]["radius_um"], 45.0)
        self.assertEqual(requests["graph"]["radius_55"]["radius_um"], 55.0)
        self.assertEqual(len(requests["graph"]["subsample"]["nodes"]), 9)
        self.assertEqual(requests["topology"]["stability"]["perturbation_replicates"], 4)
        self.assertEqual(requests["topology"]["stability"]["maximum_coordinate_jitter_um"], 1.0)
        self.assertEqual(requests["topology"]["scale_180"]["max_scale_um"], 180.0)
        self.assertEqual(requests["topology"]["scale_220"]["max_scale_um"], 220.0)

        large = [
            {
                "id": f"slide:field:{index:04d}",
                "x_um": float(index),
                "y_um": float(index % 17),
                "type_code": 1 if index % 2 == 0 else 2,
            }
            for index in range(600)
        ]
        bounded = module.field_requests(large, seed=731)
        self.assertEqual(len(bounded["topology"]["stability"]["points"]), 512)
        self.assertEqual(len(bounded["topology"]["subsample"]["points"]), 409)
        self.assertEqual(bounded["topology"]["stability"]["landmark_count"], 64)

    def test_existing_output_is_refused(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "existing"
            output.mkdir()
            with self.assertRaisesRegex(module.InterimError, "already exists"):
                module.refuse_output(output)

    def test_executor_runs_misses_then_backend_disabled_byte_equal_hits(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prepared = root / "prepared"
            prepared.mkdir()
            request = prepared / "request.json"
            request.write_text('{"request":1}\n', encoding="utf-8")
            rows = []
            for family, variant in (("graph", "baseline"), ("topology", "stability")):
                rows.append(
                    {
                        "slide_id": "slide-a",
                        "patient_id": "patient-a",
                        "field_id": "field-1",
                        "family": family,
                        "variant": variant,
                        "request": "request.json",
                        "project": f"projects/{family}",
                        "result": f"results/{family}.json",
                    }
                )
            with (prepared / "execution_manifest.csv").open("w", newline="", encoding="utf-8") as target:
                writer = csv.DictWriter(target, fieldnames=list(rows[0]), lineterminator="\n")
                writer.writeheader()
                writer.writerows(rows)
            fake = root / "fake_marklab.py"
            fake.write_text(
                "#!/usr/bin/env python3\n"
                "import json,os,pathlib,sys\n"
                "args=sys.argv[1:]\n"
                "project=pathlib.Path(args[args.index('--project')+1])\n"
                "out=pathlib.Path(args[args.index('--out')+1])\n"
                "ledger=project/'executions.jsonl'\n"
                "hit=ledger.exists()\n"
                "if os.environ.get('MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION') and not hit: sys.exit(9)\n"
                "project.mkdir(parents=True,exist_ok=True)\n"
                "if not hit: ledger.write_text('{}\\n')\n"
                "out.parent.mkdir(parents=True,exist_ok=True)\n"
                "out.write_text(json.dumps({'family':args[1]},sort_keys=True)+'\\n')\n"
                "print('cache_status='+('hit' if hit else 'miss'),file=sys.stderr)\n",
                encoding="utf-8",
            )
            os.chmod(fake, 0o755)

            miss = module.execute_manifest(prepared, fake, maximum_processes=2, replay=False)
            hit = module.execute_manifest(prepared, fake, maximum_processes=2, replay=True)

            self.assertEqual(miss["miss_count"], 2)
            self.assertEqual(hit["hit_count"], 2)
            self.assertTrue(hit["all_replay_bytes_equal"])
            self.assertTrue(hit["all_ledgers_one_execution"])

    def test_stability_is_continuous_across_four_fields_without_a_gate(self):
        module = load_module()
        baseline = {
            "field-1": {"a": 1.0, "b": 4.0},
            "field-2": {"a": 2.0, "b": 3.0},
            "field-3": {"a": 3.0, "b": 2.0},
            "field-4": {"a": 4.0, "b": 1.0},
        }
        same_ranks = {
            "field-1": {"a": 10.0, "b": 40.0},
            "field-2": {"a": 20.0, "b": 30.0},
            "field-3": {"a": 30.0, "b": 20.0},
            "field-4": {"a": 40.0, "b": 10.0},
        }

        summary = module.continuous_field_stability(baseline, [same_ranks])

        self.assertEqual(summary["field_count"], 4)
        self.assertEqual(summary["feature_count"], 2)
        self.assertEqual(summary["median_spearman"], 1.0)
        self.assertEqual(summary["q10_spearman"], 1.0)
        self.assertEqual(summary["promotion_gate"], "not_applied_in_interim_analysis")

    def test_failed_topology_attempts_keep_their_original_run_identities(self):
        module = load_module()
        root = Path("/analysis")

        attempts = module.failed_resource_attempts(root / "gastric-he-cellvit-interim-v5")

        self.assertEqual(
            [Path(attempt["root"]).name for attempt in attempts],
            ["gastric-he-cellvit-interim-v1", "gastric-he-cellvit-interim-v2"],
        )
        self.assertEqual(attempts[0]["completed_results"], 53)
        self.assertEqual(attempts[0]["failed_result_size_requests"], 11)
        self.assertEqual(attempts[1]["completed_results"], 56)
        self.assertEqual(attempts[1]["failed_result_size_requests"], 8)


if __name__ == "__main__":
    unittest.main()
