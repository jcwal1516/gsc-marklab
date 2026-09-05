"""Reproducible grouped-conformal reference fixtures, profiling and paired benchmarks.

Run with the repository's locked Python environment. Production never imports this module.
"""
from __future__ import annotations

import argparse
import cProfile
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import pstats
import statistics
import subprocess
import platform
import random
import time
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[2]
WORKER = ROOT / "workers/python/marklab_scipy_grouped_conformal_worker.py"


def fixtures():
    # The scientific package owns this frozen input; do not parse Rust test source as data.
    small_spec = ROOT / "crates/marklab-bayes/tests/fixtures/grouped_conformal/small.spec.json"
    small = json.loads(small_spec.read_text())["patients"]
    assert len(small) == 30, "original declared 30-patient fixture must remain available"
    yield "small", small, 2
    for name,n,d in [("representative",300,8),("demanding",3000,32)]:
        rows=[]
        for i in range(n):
            split="train" if i < n//2 else "calibration" if i < 3*n//4 else "test"
            # Fixed trigonometric design and deterministic noisy labels; no fitted outcomes used.
            features=[math.sin((i+1)*(j+1)*0.317)+0.3*math.cos((i+3)*(j+2)*0.173) for j in range(d)]
            rows.append(dict(patient_id=f"p{i:06}",split=split,site=f"s{i%3}",subgroup=f"g{i%2}",label=int(features[0]+0.6*math.sin(i*1.93)>0),features=features))
        yield name,rows,d


    # A prespecified replication panel tests sensitivity to the original reference failure.
    # Keep every disposition; never select a design using Rust timing or fitted effect estimates.
    for seed in range(1, 6):
        for scale,n,d in [("representative",300,8),("demanding",3000,32)]:
            rows=[]
            for i in range(n):
                split="train" if i < n//2 else "calibration" if i < 3*n//4 else "test"
                features=[math.sin((i+1+seed)*(j+1)*0.317)+0.3*math.cos((i+3)*(j+2+seed)*0.173) for j in range(d)]
                if seed == 5:
                    features[-1]=features[0]+1e-8*features[1]
                rows.append(dict(patient_id=f"p{i:06}",split=split,site=f"s{i%3}",subgroup=f"g{i%2}",label=int(features[0]+0.6*math.sin((i+seed)*1.93)>0),features=features))
            yield f"{scale}_seed_{seed}",rows,d


    # Large balanced null: nonconstant predictors, analytically zero fitted coefficients.
    # This exercises full preprocessing, quantile/prediction/coverage work with an exact oracle.
    rows=[]
    for i in range(3000):
        split="train" if i<1500 else "calibration" if i<2250 else "test"
        features=[math.sin((i//2+1)*(j+1)*0.317)+0.3*math.cos((i//2+3)*(j+2)*0.173) for j in range(32)]
        rows.append(dict(patient_id=f"p{i:06}",split=split,site=f"s{i%3}",subgroup=f"g{i%2}",label=i%2,features=features))
    yield "demanding_balanced_null",rows,32


def prepare(directory):
    directory.mkdir(parents=True, exist_ok=True)
    spec=importlib.util.spec_from_file_location("conformal_reference",WORKER)
    worker=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(worker)
    for name,patients,d in fixtures():
        patients.sort(key=lambda x:x["patient_id"])
        control=dict(patients=patients,feature_names=[f"feature_{i+1}" for i in range(d)],alpha=0.2,l2_penalty=0.1,timeout_seconds=120)
        request={"format":"marklab.scipy_grouped_conformal_request","version":1,"backend":{"name":"scipy","version":"1.18.1","python_version":"3.12","environment_lock_sha256":hashlib.sha256(WORKER.with_name("uv.lock").read_bytes()).hexdigest(),"worker_sha256":hashlib.sha256(WORKER.read_bytes()).hexdigest()},**{k:v for k,v in control.items() if k!="timeout_seconds"},"resources":{"maximum_patients":100000,"maximum_features":128,"maximum_output_bytes":16*1024*1024,"timeout_seconds":120}}
        raw=json.dumps(request,separators=(",",":"),allow_nan=False).encode()
        (directory/f"{name}.request.json").write_bytes(raw)
        (directory/f"{name}.spec.json").write_text(json.dumps(control,separators=(",",":"),allow_nan=False))
        csv="patient_id,split,site,subgroup,label,"+",".join(control["feature_names"])+"\n"
        csv+="".join(",".join([r["patient_id"],r["split"],r["site"],r["subgroup"],str(r["label"]),*[repr(v) for v in r["features"]]])+"\n" for r in patients)
        (directory/f"{name}.csv").write_text(csv)
        profile=cProfile.Profile()
        start=time.perf_counter()
        try:
            result=profile.runcall(worker.run,raw)
        except Exception as error:
            (directory/f"{name}.failure.txt").write_text(f"{type(error).__name__}: {error}\n")
            print(name,"REFERENCE FAILED",type(error).__name__,str(error),flush=True)
            continue
        elapsed=time.perf_counter()-start
        (directory/f"{name}.oracle.json").write_text(json.dumps(result,sort_keys=True,separators=(",",":"),allow_nan=False))
        with (directory/f"{name}.profile.txt").open("w") as stream:
            pstats.Stats(profile,stream=stream).sort_stats("cumulative").print_stats(25)
        print(name,"reference_profile_seconds",elapsed,"input_sha256",hashlib.sha256(raw).hexdigest(),flush=True)


def verify(reference, native):
    def compare(a,b,path):
        if isinstance(a,bool) or isinstance(a,(str,int)) or a is None:
            assert a==b,(path,a,b)
        elif isinstance(a,float):
            tolerance=1e-6*(1+abs(a)) if ".coefficients" in path or ".intercept" in path else 2e-7
            if "training_" in path or "coverage" in path: tolerance=1e-12*(1+abs(a))
            assert isinstance(b,(float,int)) and math.isfinite(b) and abs(a-b)<=tolerance,(path,a,b,tolerance)
        elif isinstance(a,list):
            assert len(a)==len(b),(path,len(a),len(b))
            for i,(x,y) in enumerate(zip(a,b)): compare(x,y,f"{path}[{i}]")
        elif isinstance(a,dict):
            assert set(a)==set(b),(path,set(a),set(b))
            for key in a: compare(a[key],b[key],f"{path}.{key}")
    for key in ("model","calibration_count","corrected_rank","nonconformity_threshold","predictions","coverage"):
        compare(reference[key],native[key],key)


def compare_warm(directory, binary):
    spec=importlib.util.spec_from_file_location("conformal_reference",WORKER)
    worker=importlib.util.module_from_spec(spec); spec.loader.exec_module(worker)
    report={"platform":platform.platform(),"python":platform.python_version(),"threads":1,"native_binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"workloads":{}}
    for request in sorted(directory.glob("*.request.json")):
        name=request.name.removesuffix(".request.json")
        raw=request.read_bytes(); spec_raw=(directory/f"{name}.spec.json").read_bytes()
        samples=[]; failures=[]
        # Warm the imported Python service before measured repetitions.
        try: worker.run(raw)
        except Exception as error:
            report["workloads"][name]={"reference_failure":str(error),"promotable":False}
            continue
        reference=None;native=None
        for repeat in range(10):
            row={}
            for implementation in (["python","rust"] if repeat%2==0 else ["rust","python"]):
                if implementation=="python":
                    start=time.perf_counter_ns();reference=worker.run(raw)
                    json.dumps(reference,separators=(",",":"),allow_nan=False).encode()
                    row[implementation]=time.perf_counter_ns()-start
                else:
                    completed=subprocess.run([str(binary),"2"],input=spec_raw,capture_output=True)
                    if completed.returncode:
                        failures.append(completed.stderr.decode());break
                    packet=json.loads(completed.stdout);native=packet["result"]
                    # First in-process service call is warmup; second is measured.
                    row[implementation]=packet["nanoseconds"][1]
            if failures: break
            verify(reference,native);samples.append(row)
        if failures:
            report["workloads"][name]={"native_failures":failures,"promotable":False};continue
        ratios=[r["python"]/r["rust"] for r in samples]
        rng=random.Random(20260905)
        boot=sorted(statistics.median(rng.choices(ratios,k=len(ratios))) for _ in range(10000))
        report["workloads"][name]={"samples_ns":samples,"python_median_ns":statistics.median(r["python"] for r in samples),"rust_median_ns":statistics.median(r["rust"] for r in samples),"median_paired_speedup":statistics.median(ratios),"paired_bootstrap_95pct":[boot[250],boot[9749]],"parity":True,"promotable":boot[250]>1.0}
        print(name,"paired warm speedup",statistics.median(ratios),"interval",boot[250],boot[9749],flush=True)
    (directory/"warm-comparison.json").write_text(json.dumps(report,indent=2,allow_nan=False)+"\n")


def compare_cold(directory, baseline, candidate):
    report={"platform":platform.platform(),"threads":1,"baseline_sha256":hashlib.sha256(baseline.read_bytes()).hexdigest(),"candidate_sha256":hashlib.sha256(candidate.read_bytes()).hexdigest(),"workloads":{}}
    env=os.environ.copy(); env.update(MARKLAB_RUNTIME_ROOT=str(ROOT),MARKLAB_PYTHON=str(ROOT/"target/pymc-venv/bin/python"))
    env.pop("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION",None)
    for name in ("small","representative_seed_2","demanding_balanced_null"):
        reference=json.loads((directory/f"{name}.oracle.json").read_text())
        samples=[]; memory=[]
        for repeat in range(10):
            row={}
            for implementation,binary in ([("python",baseline),("rust",candidate)] if repeat%2==0 else [("rust",candidate),("python",baseline)]):
                output=directory/f"{name}.{implementation}.{repeat}.{time.time_ns()}.json"
                args=[str(binary),"bayes","grouped-conformal","--input",str(directory/f"{name}.csv"),"--alpha","0.2","--l2-penalty","0.1","--timeout-seconds","120","--out",str(output)]
                run_env=env.copy()
                if implementation=="rust":
                    run_env.update(MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION="1",MARKLAB_PYTHON="/nonexistent/python",MARKLAB_RUNTIME_ROOT="/nonexistent/runtime")
                start=time.perf_counter_ns(); completed=subprocess.run(args,env=run_env,capture_output=True);row[implementation]=time.perf_counter_ns()-start
                assert completed.returncode==0,(name,implementation,completed.stderr.decode())
                result=json.loads(output.read_bytes());verify(reference,result)
                assert result["input_sha256"]==hashlib.sha256((directory/f"{name}.csv").read_bytes()).hexdigest()
            samples.append(row)
        # Separate resource runs avoid timing instrumentation inside the paired latency samples.
        for implementation,binary in [("python",baseline),("rust",candidate)]:
            resource=directory/f"{name}.{implementation}.resources.txt"
            output=directory/f"{name}.{implementation}.memory.{time.time_ns()}.json"
            if platform.system() not in ("Darwin","Linux"):
                memory.append({"implementation":implementation,"unavailable":"OS resource collector unavailable"});continue
            args=["/usr/bin/time","-l" if platform.system()=="Darwin" else "-v","-o",str(resource),str(binary),"bayes","grouped-conformal","--input",str(directory/f"{name}.csv"),"--alpha","0.2","--l2-penalty","0.1","--timeout-seconds","120","--out",str(output)]
            run_env=env.copy()
            if implementation=="rust": run_env.update(MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION="1",MARKLAB_PYTHON="/nonexistent/python",MARKLAB_RUNTIME_ROOT="/nonexistent/runtime")
            completed=subprocess.run(args,env=run_env,capture_output=True)
            assert completed.returncode==0,completed.stderr.decode()
            verify(reference,json.loads(output.read_bytes()))
            memory.append({"implementation":implementation,"os_resource_report":resource.read_text()})
        ratios=[r["python"]/r["rust"] for r in samples];rng=random.Random(20260905)
        boot=sorted(statistics.median(rng.choices(ratios,k=10)) for _ in range(10000))
        report["workloads"][name]={"samples_ns":samples,"memory":memory,"python_median_ns":statistics.median(r["python"] for r in samples),"rust_median_ns":statistics.median(r["rust"] for r in samples),"median_paired_speedup":statistics.median(ratios),"paired_bootstrap_95pct":[boot[250],boot[9749]],"parity":True,"promotable":boot[250]>1.0}
        print(name,"paired cold speedup",statistics.median(ratios),"interval",boot[250],boot[9749],flush=True)
    (directory/"cold-comparison.json").write_text(json.dumps(report,indent=2,allow_nan=False)+"\n")


def audit_reference_failures(directory, binary):
    """Check the frozen Python objective at native parameters without claiming a Python fit."""
    import numpy as np

    spec = importlib.util.spec_from_file_location("conformal_reference", WORKER)
    worker = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(worker)
    report = {
        "kind": "fixed-native-parameter Python objective/gradient and downstream re-evaluation; NOT an independent Python optimizer fit",
        "gradient_infinity_limit": 1.1e-8,
        "gradient_limit_rationale": "native 1e-8 stationarity criterion plus 1e-9 absolute floating-reduction allowance, fixed before this audit",
        "native_binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "workloads": {},
    }
    failures = json.loads((directory / "warm-comparison.json").read_text())["workloads"]
    for name, disposition in sorted(failures.items()):
        if "reference_failure" not in disposition:
            continue
        completed = subprocess.run(
            [str(binary), "1"], input=(directory / f"{name}.spec.json").read_bytes(),
            capture_output=True, timeout=150,
        )
        if completed.returncode:
            report["workloads"][name] = {"native_failure": completed.stderr.decode(), **disposition}
            continue
        native = json.loads(completed.stdout)["result"]
        parameters = np.asarray(
            [native["model"]["intercept"], *native["model"]["coefficients"]], dtype=np.float64,
        )
        evaluated = {}

        def evaluate(fun, initial, **kwargs):
            assert kwargs["method"] == "BFGS" and kwargs["jac"] is True
            assert kwargs["options"] == {"maxiter": 1000, "gtol": 1e-8}
            value, gradient = fun(parameters)
            evaluated.update(
                objective=float(value), gradient=[float(x) for x in gradient],
                gradient_infinity=float(np.max(np.abs(gradient))),
            )
            return SimpleNamespace(success=True, x=parameters, message="fixed-parameter evaluation only")

        # Only this fresh module's optimizer call is replaced. The frozen objective, preparation,
        # quantile, predictions and coverage code still run. No reference source file is edited.
        worker.minimize = evaluate
        fixed_reference = worker.run((directory / f"{name}.request.json").read_bytes())
        verify(fixed_reference, native)
        assert np.isfinite(evaluated["objective"])
        assert evaluated["gradient_infinity"] <= report["gradient_infinity_limit"], (name, evaluated)
        evaluated.update(
            reference_failure=disposition["reference_failure"],
            fixed_parameter_downstream_parity=True, independent_python_optimizer_parity=False,
        )
        report["workloads"][name] = evaluated
        print(name, "fixed-parameter gradient infinity", evaluated["gradient_infinity"], flush=True)
    (directory / "failed-reference-fixed-parameter-audit.json").write_text(
        json.dumps(report, indent=2, allow_nan=False) + "\n",
    )


if __name__ == "__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("--output",type=Path,default=ROOT/"target/native-migration/conformal")
    parser.add_argument("--native-warm",type=Path)
    parser.add_argument("--baseline",type=Path)
    parser.add_argument("--candidate",type=Path)
    parser.add_argument("--audit-failures",type=Path,help="Native example binary for fixed-parameter reference evaluation")
    args=parser.parse_args()
    # Set these before importing any array package, matching the production worker policy.
    for key in ("OMP_NUM_THREADS","OPENBLAS_NUM_THREADS","MKL_NUM_THREADS"):
        os.environ[key]="1"
    if args.baseline and args.candidate:
        compare_cold(args.output,args.baseline.resolve(),args.candidate.resolve())
    elif args.audit_failures:
        audit_reference_failures(args.output,args.audit_failures.resolve())
    elif args.native_warm:
        compare_warm(args.output,args.native_warm.resolve())
    else:
        prepare(args.output)
