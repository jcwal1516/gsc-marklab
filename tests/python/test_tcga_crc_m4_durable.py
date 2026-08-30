import csv
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import stat
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "workers/python/marklab_tcga_crc_m4_durable.py"


FAKE_MARKLAB = r'''#!/usr/bin/env python3
import hashlib, json, os
from pathlib import Path
import sys
args=sys.argv[1:]
def value(name): return args[args.index(name)+1]
project=Path(value("--project")); output=Path(value("--out")); stored=project/"stored.json"
if os.environ.get("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION") == "1":
    if not stored.is_file(): raise SystemExit("disabled miss")
    output.parent.mkdir(parents=True,exist_ok=True); output.write_bytes(stored.read_bytes())
    print("project vector-semivariogram cache_status=hit",file=sys.stderr); raise SystemExit(0)
document={"format":"marklab.vector_semivariogram","version":1,"input_sha256":hashlib.sha256(Path(value("--input")).read_bytes()).hexdigest(),"bins_sha256":hashlib.sha256(Path(value("--bins")).read_bytes()).hexdigest(),"weights_sha256":None,"coordinate_unit":"micrometer","object_count":3,"embedding_dimension":2,"feature_names":["embedding_0","embedding_1"],"pair_visits":3,"weighting":"unit","rotation_invariant":True,"curve":[]}
encoded=(json.dumps(document,indent=2)+"\n").encode(); project.mkdir(parents=True,exist_ok=True); stored.write_bytes(encoded); (project/"executions.jsonl").write_text("{}\n"); output.parent.mkdir(parents=True,exist_ok=True); output.write_bytes(encoded); print("project vector-semivariogram cache_status=miss",file=sys.stderr)
'''


class M4DurableTest(unittest.TestCase):
    def test_patient_inputs_miss_replay_and_match_frozen_results(self):
        spec = importlib.util.spec_from_file_location("m4_durable", MODULE)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            input_root = root / "input"
            reference = root / "reference"
            (input_root / "patients").mkdir(parents=True)
            reference.mkdir()
            bins = input_root / "bins.csv"
            bins.write_text("bin_id,lower_um,upper_um\nnear,0,2\n", encoding="utf-8")
            rows = []
            fake = root / "marklab"
            fake.write_text(FAKE_MARKLAB, encoding="utf-8")
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            for index in range(4):
                patient = f"P{index}"
                path = input_root / "patients" / f"{patient}.csv"
                path.write_text(
                    "object_id,x_um,y_um,embedding_0,embedding_1\na,0,0,0,0\nb,1,0,1,0\nc,2,0,1,1\n",
                    encoding="utf-8",
                )
                rows.append({"patient_id": patient, "cell_count": 3, "input": f"patients/{patient}.csv", "input_sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
                project = root / f"reference-project-{patient}"
                output = reference / f"{patient}.json"
                import subprocess
                subprocess.run([str(fake),"project","vector-semivariogram","--project",str(project),"--input",str(path),"--bins",str(bins),"--out",str(output),"--maximum-points","3","--maximum-dimension","2","--maximum-pair-visits","3","--memory-budget-mib","16"],check=True)
            with (input_root / "patients.csv").open("w", newline="", encoding="utf-8") as target:
                writer = csv.DictWriter(target, fieldnames=list(rows[0]), lineterminator="\n"); writer.writeheader(); writer.writerows(rows)
            (input_root / "admission.json").write_text(json.dumps({"patient_count":4,"raw_embedding_width":2})+"\n")
            output = root / "durable"
            module.execute(input_root, reference, fake, output, 2, 30, replay=False)
            (output / "execution_manifest.json").unlink()
            resumed = module.execute(
                input_root,
                reference,
                fake,
                output,
                2,
                30,
                replay=False,
                resume_completed=True,
            )
            replay = module.execute(input_root, reference, fake, output, 2, 30, replay=True)
            self.assertEqual(resumed["resumed_completed_count"], 4)
            self.assertEqual(replay["cache_status_counts"], {"hit": 4})
            self.assertTrue(replay["all_replay_bytes_equal"])
            self.assertTrue(replay["all_reference_compatible"])
            self.assertTrue(replay["all_ledgers_one_execution"])
            self.assertTrue(module.results_compatible(1.0, math.nextafter(1.0, 2.0)))
            self.assertFalse(
                module.results_compatible(
                    1.0, math.nextafter(math.nextafter(1.0, 2.0), 2.0)
                )
            )


if __name__ == "__main__":
    unittest.main()
