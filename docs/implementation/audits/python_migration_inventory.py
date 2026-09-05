"""Generate a traceable inventory; reference matches are evidence, not proof of reachability."""
from __future__ import annotations
import ast
import csv
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
BASELINE = "f784302"

NATIVE_ADMISSIONS = {
    "marklab_scipy_grouped_conformal_worker": "EMB-CONFORMAL-01 / RUST-MIGRATION-01",
    "marklab_scipy_prediction_calibration_worker": "EMB-CALIBRATE-01 / RUST-MIGRATION-01 (two legacy cold failures retained)",
    "marklab_scipy_late_fusion_worker": "EMB-LATE-FUSION-01 / RUST-MIGRATION-01 (three failed reference fits retained)",
    "marklab_scipy_partial_transport_worker": "REG-PARTIAL-OT-01 / RUST-MIGRATION-01 (zero-capacity Python failure; analytic native oracle)",
    "marklab_scipy_predictive_stacking_worker": "EMB-STACKING-01 / RUST-MIGRATION-01 (complete jackknife; nine frozen cases)",
    "marklab_scipy_mixture_of_experts_worker": "EMB-MOE-01 / RUST-MIGRATION-01 (DEC-0423 demanding precision; constant-context failure retained)",
    "marklab_scipy_pcca_em_worker": "MM-PCCA-01 / RUST-MIGRATION-01 (initial slowdown retained; exact sufficient-statistic optimization)",
}


def owner(name):
    if name == "marklab_scipy_partial_transport_worker": return "marklab-bayes (reviewed native transport owner)"
    if any(x in name for x in ("gudhi", "topology", "skimage_raster")): return "marklab-topology"
    if any(x in name for x in ("registration", "simpleitk", "lddmm", "svf", "atlas", "serial_stack", "deformation", "landmark", "fgw", "fused_gromov", "partial_transport")): return "registration / marklab-spatial3d"
    if "point_set_generators" in name or "clone_models" in name: return "marklab-simulation"
    if "sbi_neural" in name or "generative_validation" in name or "neural_point_process" in name: return "marklab-sbi / marklab-simulation"
    if "causal" in name: return "marklab-causal"
    if any(x in name for x in ("mofapy2", "factor", "modality_dropout", "joint_pathology", "multimodal_validation")) and not any(x in name for x in ("lgcp", "pymc")): return "marklab-embeddings"
    if any(name.startswith(x) for x in ("marklab_crc", "marklab_tcga", "marklab_cellvit", "marklab_gastric", "marklab_schurch", "crc_spatial")): return "library application; existing data/cohort/scientific owners"
    if name == "marklab_client": return "library application / bounded H5AD ingestion"
    if name == "marklab_backend_doctor": return "runtime delivery; remove after final migration"
    if "advanced3d" in name: return "marklab-spatial3d"
    return "marklab-bayes"


def main():
    revision=subprocess.check_output(["git","rev-parse",BASELINE],cwd=ROOT,text=True).strip()
    target=ROOT/"docs/implementation/PYTHON_MIGRATION_INVENTORY.csv"
    # The frozen revision fixes these bytes; reuse its already-recorded source identities.
    previous_hashes = {}
    if target.exists():
        with target.open(newline="") as previous:
            previous_hashes = {row["python_source"]: row["sha256"] for row in csv.DictReader(previous)
                               if row["baseline_revision"] == revision}
    tracked=subprocess.check_output(["git","ls-tree","-r","--name-only",revision],cwd=ROOT,text=True).splitlines()
    source_paths=[p for p in tracked if p.endswith((".rs",".py",".md",".yml"))]
    # Read frozen Git blobs in one batch. Working-tree migrations must not erase their old callers.
    with tempfile.TemporaryFile() as requests:
        requests.write("".join(f"{revision}:{p}\n" for p in source_paths).encode())
        requests.seek(0)
        blobs=subprocess.check_output(["git","cat-file","--batch"],cwd=ROOT,stdin=requests,timeout=60,env={**os.environ,"GIT_NO_LAZY_FETCH":"1"})
    stream=io.BytesIO(blobs)
    sources={}
    for path in source_paths:
        _,kind,size=stream.readline().split()
        assert kind==b"blob",path
        sources[path]=stream.read(int(size)).decode()
        assert stream.read(1)==b"\n",path
    paths=[p for p in tracked if p.endswith(".py") and (p.startswith("workers/python/") or p=="clients/python/marklab_client.py") and not Path(p).name.startswith("test_")]
    output=io.StringIO()
    writer=csv.writer(output,lineterminator="\n")
    writer.writerow(["baseline_revision","python_source","sha256","import_roots","production_reference_matches","test_reference_matches","contract_reference_matches","proposed_owner_review_required","migration_status","workload_admission"])
    for path in sorted(paths):
        stem=Path(path).stem
        imports=set()
        for node in ast.walk(ast.parse(sources[path])):
            if isinstance(node,ast.Import): imports.update(a.name.split('.')[0] for a in node.names)
            if isinstance(node,ast.ImportFrom): imports.add((node.module or '').split('.')[0])
        matches=[p for p,text in sources.items() if p!=path and stem in text]
        production=[p for p in matches if p.endswith((".rs",".py")) and not p.startswith(("tests/","docs/")) and "/tests/" not in p and not Path(p).name.startswith("test_")]
        tests=[p for p in matches if p.startswith("tests/") or "/tests/" in p or Path(p).name.startswith("test_")]
        contracts=[p for p in matches if p.startswith("docs/implementation/")]
        digest = previous_hashes.get(path)
        if digest is None:
            digest = hashlib.sha256(sources[path].encode()).hexdigest()
        status = "native: bounded parity and synthetic performance gates passed" if stem in NATIVE_ADMISSIONS else "pending"
        admission = NATIVE_ADMISSIONS.get(stem, "existing caller fixtures; not yet individually admitted")
        writer.writerow([revision,path,digest,";".join(sorted(imports)),";".join(production),";".join(tests),";".join(contracts),owner(stem),status,admission])
    target.write_text(output.getvalue())
    print(f"{len(paths)} Python sources inventoried; unmatched/dynamic callers require review before porting")


if __name__ == "__main__": main()
