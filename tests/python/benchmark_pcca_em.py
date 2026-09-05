#!/usr/bin/env python3
"""Freeze pCCA EM reference cases and run paired complete-workflow timings."""

from __future__ import annotations

import argparse
import cProfile
import hashlib
import importlib.util
import io
import json
import math
import os
import pathlib
import platform
import pstats
import subprocess
import sys
import tempfile
import time

from benchmark_late_fusion import summary


ROOT = pathlib.Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "crates/marklab-bayes/tests/fixtures/pcca_em"
WORKER = ROOT / "workers/python/marklab_scipy_pcca_em_worker.py"
PYTHON = ROOT / "target/pymc-venv/bin/python"
OUTPUT = ROOT / "target/native-migration/pcca-em"


def design(dx: int, dy: int) -> dict:
    return {
        "entity_level": "patient",
        "modality_x": {
            "id": "morphology",
            "measurement_status": "measured",
            "likelihood": "gaussian",
            "feature_names": [f"x{index + 1}" for index in range(dx)],
        },
        "modality_y": {
            "id": "ihc",
            "measurement_status": "measured",
            "likelihood": "gaussian",
            "feature_names": [f"y{index + 1}" for index in range(dy)],
        },
        "missingness_assumption": "complete_paired_rows",
        "coordinate_frame": None,
    }


def synthetic_rows(count: int, dx: int, dy: int, latent: int) -> list[dict]:
    rows = []
    train = max(8, count * 3 // 4)
    for index in range(count):
        factors = [
            math.sin((index + 1) * (component + 1) * 0.071)
            + 0.7 * math.cos((index + 2) * (component + 2) * 0.037)
            + (index / max(1, count - 1) - 0.5) * (component + 1) * 0.3
            for component in range(latent)
        ]
        x = [
            sum(
                factors[component]
                * math.sin((feature + 1) * (component + 1) * 0.53)
                for component in range(latent)
            )
            + 0.025 * math.sin((index + 3) * (feature + 2) * 0.19)
            for feature in range(dx)
        ]
        y = [
            sum(
                factors[component]
                * math.cos((feature + 2) * (component + 1) * 0.41)
                for component in range(latent)
            )
            + 0.025 * math.cos((index + 5) * (feature + 1) * 0.17)
            for feature in range(dy)
        ]
        rows.append(
            {
                "entity_id": f"p{index:05}",
                "split": "train" if index < train else "test",
                "x": x,
                "y": y,
            }
        )
    return rows


def cases() -> dict[str, dict]:
    small_rows = []
    for index in range(12):
        z = index - 5.5
        noise = 0.08 if index % 2 == 0 else -0.08
        small_rows.append(
            {
                "entity_id": f"p{index:02}",
                "split": "train" if index < 8 else "test",
                "x": [z + noise, 0.5 * z - noise],
                "y": [2.0 * z - noise, -z + noise],
            }
        )
    return {
        "small": {
            "design": design(2, 2),
            "rows": small_rows,
            "latent_dimensions": 1,
            "regularization": 1e-6,
            "noise_floor": 1e-6,
            "maximum_iterations": 200,
            "convergence_tolerance": 1e-9,
            "timeout_seconds": 30,
        },
        "representative": {
            "design": design(8, 6),
            "rows": synthetic_rows(256, 8, 6, 2),
            "latent_dimensions": 2,
            "regularization": 1e-5,
            "noise_floor": 1e-5,
            "maximum_iterations": 500,
            "convergence_tolerance": 1e-8,
            "timeout_seconds": 60,
        },
        "demanding": {
            "design": design(16, 16),
            "rows": synthetic_rows(2000, 16, 16, 4),
            "latent_dimensions": 4,
            "regularization": 1e-5,
            "noise_floor": 1e-5,
            "maximum_iterations": 750,
            "convergence_tolerance": 1e-8,
            "timeout_seconds": 120,
        },
    }


def worker_request(spec: dict) -> dict:
    return {
        "format": "marklab.scipy_pcca_em_request",
        "version": 1,
        "backend": {
            "name": "numpy_scipy",
            "version": "numpy-2.4.6+scipy-1.18.1",
            "python_version": "3.12",
            "license": "BSD-3-Clause",
            "environment_lock_sha256": hashlib.sha256(
                (ROOT / "workers/python/uv.lock").read_bytes()
            ).hexdigest(),
            "worker_sha256": hashlib.sha256(WORKER.read_bytes()).hexdigest(),
        },
        **{key: value for key, value in spec.items() if key != "timeout_seconds"},
    }


def prepare() -> None:
    FIXTURES.mkdir(parents=True, exist_ok=True)
    environment = {
        "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
        "LC_ALL": "C",
        "PYTHONHASHSEED": "0",
        "PYTHONNOUSERSITE": "1",
        "PYTHONDONTWRITEBYTECODE": "1",
        "OMP_NUM_THREADS": "1",
        "OPENBLAS_NUM_THREADS": "1",
        "MKL_NUM_THREADS": "1",
    }
    for name, spec in cases().items():
        request = worker_request(spec)
        request_bytes = json.dumps(
            request, allow_nan=False, separators=(",", ":"), sort_keys=True
        ).encode()
        completed = subprocess.run(
            [str(PYTHON), "-P", "-s", "-B", str(WORKER)],
            input=request_bytes,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=environment,
            timeout=spec["timeout_seconds"] + 5,
            check=False,
        )
        if completed.returncode != 0:
            raise RuntimeError(f"{name}: {completed.stderr.decode()}")
        (FIXTURES / f"{name}.spec.json").write_text(
            json.dumps(spec, allow_nan=False, indent=2, sort_keys=True) + "\n"
        )
        (FIXTURES / f"{name}.request.json").write_bytes(request_bytes)
        (FIXTURES / f"{name}.oracle.json").write_bytes(completed.stdout)
        print(name, json.loads(completed.stdout)["diagnostics"])


def load_worker():
    loader = importlib.util.spec_from_file_location("pcca_reference", WORKER)
    module = importlib.util.module_from_spec(loader)
    loader.loader.exec_module(module)
    return module


def python_run(module, request: bytes) -> dict:
    previous_stdin, previous_stdout = module.sys.stdin, module.sys.stdout
    stdin = io.TextIOWrapper(io.BytesIO(request), encoding="utf-8")
    stdout = io.StringIO()
    try:
        module.sys.stdin, module.sys.stdout = stdin, stdout
        module.main()
        return json.loads(stdout.getvalue())
    finally:
        module.sys.stdin, module.sys.stdout = previous_stdin, previous_stdout


def native_wire(specification: bytes) -> bytes:
    return json.dumps(
        {"json": specification.decode()}, separators=(",", ":"), allow_nan=False
    ).encode()


def close(actual: float, expected: float, tolerance: float = 3e-5) -> None:
    assert math.isfinite(actual)
    assert abs(actual - expected) <= tolerance * (1.0 + abs(expected)), (
        actual,
        expected,
    )


def implied_covariance(result: dict) -> list[list[float]]:
    parameters = result["parameters"]
    loadings = parameters["loadings_x"] + parameters["loadings_y"]
    noise = parameters["noise_diagonal_x"] + parameters["noise_diagonal_y"]
    return [
        [
            sum(a * b for a, b in zip(loadings[row], loadings[column]))
            + (noise[row] if row == column else 0.0)
            for column in range(len(loadings))
        ]
        for row in range(len(loadings))
    ]


def verify(oracle: dict, actual: dict) -> None:
    for key in ["format", "design", "heldout_row_count", "claim_status"]:
        assert actual[key] == oracle[key], key
    for key in ["mean_x", "scale_x", "mean_y", "scale_y"]:
        for observed, expected in zip(
            actual["standardization"][key], oracle["standardization"][key]
        ):
            close(observed, expected, 2e-10)
    for key in ["noise_diagonal_x", "noise_diagonal_y"]:
        for observed, expected in zip(
            actual["parameters"][key], oracle["parameters"][key]
        ):
            close(observed, expected)
    for observed, expected in zip(
        actual["canonical_correlations"], oracle["canonical_correlations"]
    ):
        close(observed, expected)
    close(
        actual["heldout_cross_view_rmse_y_from_x"],
        oracle["heldout_cross_view_rmse_y_from_x"],
    )
    assert actual["diagnostics"]["iterations"] == oracle["diagnostics"]["iterations"]
    assert actual["diagnostics"]["converged"] == oracle["diagnostics"]["converged"]
    for observed, expected in zip(
        actual["diagnostics"]["log_likelihood_trace"],
        oracle["diagnostics"]["log_likelihood_trace"],
    ):
        close(observed, expected, 2e-5)
    for observed_row, expected_row in zip(
        implied_covariance(actual), implied_covariance(oracle)
    ):
        for observed, expected in zip(observed_row, expected_row):
            close(observed, expected)
    actual_loadings = actual["parameters"]["loadings_x"] + actual["parameters"]["loadings_y"]
    oracle_loadings = oracle["parameters"]["loadings_x"] + oracle["parameters"]["loadings_y"]
    for actual_score, oracle_score in zip(
        actual["posterior_scores"], oracle["posterior_scores"]
    ):
        assert actual_score["entity_id"] == oracle_score["entity_id"]
        assert actual_score["split"] == oracle_score["split"]
        for feature in range(len(actual_loadings)):
            observed = sum(
                score * loading
                for score, loading in zip(actual_score["mean"], actual_loadings[feature])
            )
            expected = sum(
                score * loading
                for score, loading in zip(oracle_score["mean"], oracle_loadings[feature])
            )
            close(observed, expected)


def profile_reference() -> None:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    module = load_worker()
    for name in ["small", "representative"]:
        profile = cProfile.Profile()
        profile.runcall(
            python_run, module, (FIXTURES / f"{name}.request.json").read_bytes()
        )
        with (OUTPUT / f"{name}.python-profile.txt").open("w") as stream:
            pstats.Stats(profile, stream=stream).sort_stats("cumulative").print_stats(30)


def benchmark(
    warm_binary: pathlib.Path,
    reference_binary: pathlib.Path,
    native_binary: pathlib.Path,
) -> bool:
    module = load_worker()
    report = {
        "platform": platform.platform(),
        "python": platform.python_version(),
        "repetitions": 10,
        "thread_budget": 1,
        "warm_boundary": "Python JSON worker and Rust JSON application; both include parse, full EM, every diagnostic and serialization",
        "cases": {},
        "identities": {},
    }
    for label, path in [
        ("worker", WORKER),
        ("lock", ROOT / "workers/python/uv.lock"),
        ("reference_binary", reference_binary),
        ("native_binary", native_binary),
        ("warm_binary", warm_binary),
    ]:
        report["identities"][label] = {
            "path": str(path),
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        }
    environment = os.environ.copy()
    environment.update(
        MARKLAB_RUNTIME_ROOT=str(ROOT),
        MARKLAB_PYTHON=str(PYTHON),
        OMP_NUM_THREADS="1",
        OPENBLAS_NUM_THREADS="1",
        MKL_NUM_THREADS="1",
    )
    environment.pop("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", None)
    for name, spec in cases().items():
        specimen = FIXTURES / f"{name}.spec.json"
        specification = specimen.read_bytes()
        request = (FIXTURES / f"{name}.request.json").read_bytes()
        oracle = json.loads((FIXTURES / f"{name}.oracle.json").read_bytes())
        case = {
            "rows": len(spec["rows"]),
            "dx": len(spec["design"]["modality_x"]["feature_names"]),
            "dy": len(spec["design"]["modality_y"]["feature_names"]),
            "latent": spec["latent_dimensions"],
        }
        report["cases"][name] = case
        with tempfile.TemporaryDirectory(prefix="marklab-pcca-") as temporary:
            for phase in ["warm", "cold"]:
                samples = []
                try:
                    for repetition in range(10):
                        pair = {}
                        labels = ["python", "rust"]
                        if repetition % 2:
                            labels.reverse()
                        for label in labels:
                            if phase == "warm" and label == "python":
                                started = time.perf_counter_ns()
                                value = python_run(module, request)
                                json.dumps(value, allow_nan=False, separators=(",", ":")).encode()
                                elapsed = time.perf_counter_ns() - started
                            elif phase == "warm":
                                packet = json.loads(
                                    subprocess.run(
                                        [str(warm_binary), "2"],
                                        input=native_wire(specification),
                                        capture_output=True,
                                        check=True,
                                        env=environment,
                                        timeout=spec["timeout_seconds"] + 5,
                                    ).stdout
                                )
                                value = packet["result"]
                                elapsed = packet["nanoseconds"][1]
                            else:
                                output = pathlib.Path(temporary) / f"{phase}-{repetition}-{label}.json"
                                binary = reference_binary if label == "python" else native_binary
                                started = time.perf_counter_ns()
                                subprocess.run(
                                    [
                                        str(binary),
                                        "multimodal",
                                        "pcca",
                                        "--input",
                                        str(specimen),
                                        "--out",
                                        str(output),
                                    ],
                                    capture_output=True,
                                    check=True,
                                    env=environment,
                                    timeout=spec["timeout_seconds"] + 5,
                                )
                                elapsed = time.perf_counter_ns() - started
                                value = json.loads(output.read_bytes())
                            verify(oracle, value)
                            pair[label] = elapsed
                        samples.append(pair)
                    case[phase] = summary(samples)
                    print(
                        name,
                        phase,
                        case[phase]["median_paired_speedup"],
                        case[phase]["paired_bootstrap_95pct"],
                        flush=True,
                    )
                    if phase == "cold":
                        memory = {}
                        for label, binary in [
                            ("python", reference_binary),
                            ("rust", native_binary),
                        ]:
                            output = pathlib.Path(temporary) / f"rss-{label}.json"
                            run = subprocess.run(
                                [
                                    "/usr/bin/time",
                                    "-l",
                                    str(binary),
                                    "multimodal",
                                    "pcca",
                                    "--input",
                                    str(specimen),
                                    "--out",
                                    str(output),
                                ],
                                capture_output=True,
                                check=True,
                                env=environment,
                                timeout=spec["timeout_seconds"] + 5,
                            )
                            verify(oracle, json.loads(output.read_bytes()))
                            memory[label] = run.stderr.decode()
                        case["rss_time_l"] = memory
                except Exception as error:
                    case[phase] = {
                        "samples_ns": samples,
                        "failure": str(error),
                        "stderr": (getattr(error, "stderr", None) or b"").decode(
                            errors="replace"
                        ),
                        "promotable": False,
                    }
                OUTPUT.mkdir(parents=True, exist_ok=True)
                (OUTPUT / "comparison.json").write_text(
                    json.dumps(report, indent=2, allow_nan=False, sort_keys=True) + "\n"
                )
    return all(
        case.get(phase, {}).get("promotable", False)
        for case in report["cases"].values()
        for phase in ["warm", "cold"]
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--prepare", action="store_true")
    parser.add_argument("--profile", action="store_true")
    parser.add_argument("--benchmark", action="store_true")
    parser.add_argument("--warm-binary", type=pathlib.Path)
    parser.add_argument("--reference-binary", type=pathlib.Path)
    parser.add_argument("--native-binary", type=pathlib.Path)
    arguments = parser.parse_args()
    if arguments.prepare:
        prepare()
    elif arguments.profile:
        profile_reference()
    elif arguments.benchmark:
        if (
            arguments.warm_binary is None
            or arguments.reference_binary is None
            or arguments.native_binary is None
        ):
            parser.error("--benchmark requires warm, reference, and native binaries")
        if not benchmark(
            arguments.warm_binary.resolve(),
            arguments.reference_binary.resolve(),
            arguments.native_binary.resolve(),
        ):
            raise SystemExit(1)
    else:
        parser.error("select --prepare or --benchmark")


if __name__ == "__main__":
    main()
