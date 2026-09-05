"""Frozen mixture-of-experts oracles and paired complete-workflow measurements."""

from __future__ import annotations

import argparse
import cProfile
import csv
import hashlib
import importlib.util
import io
import json
import math
import os
from pathlib import Path
import platform
import pstats
import random
import statistics
import struct
import subprocess
import tempfile
import time

import numpy as np
import scipy.optimize


ROOT = Path(__file__).resolve().parents[2]
WORKER = ROOT / "workers/python/marklab_scipy_mixture_of_experts_worker.py"
FIXTURES = ROOT / "crates/marklab-bayes/tests/fixtures/mixture_of_experts"
OUTPUT = ROOT / "target/native-migration/mixture-of-experts"


def _patient(index: int, n: int, contexts: int, experts: int, edge: str | None) -> dict:
    split = "gate_train" if index < n // 3 else "calibration" if index < 2 * n // 3 else "test"
    signal = math.sin((index + 1) * 0.371) + 0.35 * math.cos((index + 1) * 1.173)
    label = int(signal > 0.0)
    context = [
        signal if column == 0 else math.sin((index + 1) * (0.113 + column * 0.071))
        for column in range(contexts)
    ]
    probabilities = []
    for expert in range(experts):
        preferred = int((context[0] >= 0.0)) == (expert % 2)
        strength = 0.88 if preferred else 0.58
        probability = strength if label else 1.0 - strength
        probability += 0.012 * math.sin((index + 1) * (expert + 2))
        probabilities.append(min(max(probability, 0.01), 0.99))
    if experts > 2 and index % 11 == 0:
        probabilities[2 + index % (experts - 2)] = None
    if edge == "availability" and split == "test" and index % 4 == 0:
        probabilities = [None] * experts
        probabilities[index % experts] = 0.82 if label else 0.18
    if edge == "constant_context":
        context[-1] = 1.0
    return {
        "patient_id": f"p{index:06}",
        "split": split,
        "expert_prediction_source": "patient_level_out_of_fold",
        "label": label,
        "context": context,
        "expert_probabilities": probabilities,
    }


def fixture_specs():
    cases = [
        ("small", 30, 1, 2, "availability"),
        ("representative", 300, 3, 4, None),
        ("demanding", 3000, 4, 4, None),
        ("maximum_features", 30, 16, 8, "availability"),
        ("constant_context", 30, 2, 2, "constant_context"),
    ]
    for name, n, contexts, experts, edge in cases:
        yield name, {
            "patients": [_patient(i, n, contexts, experts, edge) for i in range(n)],
            "context_names": [f"context_signal_{i}" for i in range(contexts)],
            "expert_names": [f"expert_{i}" for i in range(experts)],
            "l2_penalty": 0.1,
            "entropy_regularization": 0.01,
            "ood_validation_quantile": 0.9,
            "timeout_seconds": 120,
        }


def reference_module():
    module = importlib.util.spec_from_file_location("moe_reference", WORKER)
    instance = importlib.util.module_from_spec(module)
    module.loader.exec_module(instance)
    return instance


def request_for(spec: dict) -> dict:
    return {
        "format": "marklab.scipy_mixture_of_experts_request",
        "version": 1,
        "backend": {
            "name": "scipy",
            "version": "1.18.1",
            "python_version": "3.12",
            "environment_lock_sha256": hashlib.sha256(WORKER.with_name("uv.lock").read_bytes()).hexdigest(),
            "worker_sha256": hashlib.sha256(WORKER.read_bytes()).hexdigest(),
        },
        "patients": spec["patients"],
        "context_names": spec["context_names"],
        "expert_names": spec["expert_names"],
        "l2_penalty": spec["l2_penalty"],
        "entropy_regularization": spec["entropy_regularization"],
        "ood_validation_quantile": spec["ood_validation_quantile"],
        "resources": {
            "maximum_patients": 10000,
            "maximum_context_features": 16,
            "maximum_experts": 8,
            "maximum_output_bytes": 16777216,
            "timeout_seconds": spec["timeout_seconds"],
        },
    }


def prepare() -> None:
    FIXTURES.mkdir(parents=True, exist_ok=True)
    OUTPUT.mkdir(parents=True, exist_ok=True)
    worker = reference_module()
    for name, spec in fixture_specs():
        raw = json.dumps(request_for(spec), separators=(",", ":"), allow_nan=False).encode()
        (FIXTURES / f"{name}.request.json").write_bytes(raw)
        (FIXTURES / f"{name}.spec.json").write_text(
            json.dumps(spec, separators=(",", ":"), allow_nan=False) + "\n"
        )
        profile = cProfile.Profile()
        try:
            result = profile.runcall(worker.run, raw)
            (FIXTURES / f"{name}.oracle.json").write_text(
                json.dumps(result, separators=(",", ":"), allow_nan=False) + "\n"
            )
            print(name, "reference passed", flush=True)
        except Exception as error:
            (OUTPUT / f"{name}.failure.txt").write_text(f"{type(error).__name__}: {error}\n")
            print(name, "REFERENCE FAILED", error, flush=True)
        with (OUTPUT / f"{name}.profile.txt").open("w") as stream:
            pstats.Stats(profile, stream=stream).sort_stats("cumulative").print_stats(25)


def csv_text(spec: dict) -> str:
    stream = io.StringIO()
    writer = csv.writer(stream, lineterminator="\n")
    writer.writerow(
        [
            "patient_id",
            "split",
            "expert_prediction_source",
            "label",
            *spec["context_names"],
            *spec["expert_names"],
        ]
    )
    for patient in spec["patients"]:
        writer.writerow(
            [
                patient["patient_id"],
                patient["split"],
                patient["expert_prediction_source"],
                patient["label"],
                *[repr(value) for value in patient["context"]],
                *["" if value is None else repr(value) for value in patient["expert_probabilities"]],
            ]
        )
    return stream.getvalue()


def native_wire(spec: dict) -> bytes:
    def bits(value: float) -> int:
        return int.from_bytes(struct.pack("<d", value), "little")

    return json.dumps(
        {
            "csv": csv_text(spec),
            "l2_penalty_bits": bits(spec["l2_penalty"]),
            "entropy_regularization_bits": bits(spec["entropy_regularization"]),
            "ood_validation_quantile_bits": bits(spec["ood_validation_quantile"]),
            "timeout_seconds": spec["timeout_seconds"],
        },
        separators=(",", ":"),
    ).encode()


def verify(reference: dict, candidate: dict, workload: str) -> None:
    def compare(left, right, path: str) -> None:
        if isinstance(left, dict):
            assert set(left) == set(right), (path, set(left), set(right))
            for key in left:
                compare(left[key], right[key], f"{path}.{key}")
        elif isinstance(left, list):
            assert len(left) == len(right), (path, len(left), len(right))
            for index, (a, b) in enumerate(zip(left, right)):
                compare(a, b, f"{path}[{index}]")
        elif isinstance(left, float):
            if workload == "demanding" and any(
                token in path for token in ("coefficients", "gating_weights")
            ):
                scale = 1e-4
            elif workload == "demanding" and (
                any(token in path for token in ("intercept", "slope", "raw_probability", "metrics"))
                or path.endswith(".probability")
            ):
                scale = 1e-5
            elif any(token in path for token in ("coefficients", "intercept", "slope")):
                scale = 2e-5
            else:
                scale = 2e-6
            tolerance = scale * (1.0 + abs(left))
            assert math.isfinite(right) and abs(left - right) <= tolerance, (
                path,
                left,
                right,
                tolerance,
            )
        else:
            assert left == right, (path, left, right)

    for key in ("model", "calibrator", "ood_threshold", "predictions", "metrics"):
        compare(reference[key], candidate[key], key)


def _summary(samples: list[dict[str, int]]) -> dict:
    ratios = [sample["python"] / sample["rust"] for sample in samples]
    generator = random.Random(20260905)
    bootstrap = sorted(
        statistics.median(generator.choices(ratios, k=len(ratios))) for _ in range(10000)
    )
    return {
        "samples_ns": samples,
        "python_median_ns": statistics.median(sample["python"] for sample in samples),
        "rust_median_ns": statistics.median(sample["rust"] for sample in samples),
        "median_paired_speedup": statistics.median(ratios),
        "paired_bootstrap_95pct": [bootstrap[250], bootstrap[9749]],
        "parity": True,
        "promotable": bootstrap[250] > 1.0,
    }


def measure(warm: Path, baseline: Path, candidate: Path) -> None:
    worker = reference_module()
    report = {
        "platform": platform.platform(),
        "python": platform.python_version(),
        "threads": 1,
        "workloads": {},
        "identities": {},
    }
    for label, path in (
        ("worker", WORKER),
        ("lock", WORKER.with_name("uv.lock")),
        ("baseline", baseline),
        ("candidate", candidate),
        ("warm", warm),
    ):
        report["identities"][label] = {
            "path": str(path),
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        }
    environment = os.environ.copy()
    environment.update(
        MARKLAB_RUNTIME_ROOT=str(ROOT),
        MARKLAB_PYTHON=str(ROOT / "target/pymc-venv/bin/python"),
        OMP_NUM_THREADS="1",
        OPENBLAS_NUM_THREADS="1",
        MKL_NUM_THREADS="1",
    )
    environment.pop("MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION", None)
    for name in ("small", "representative", "demanding", "maximum_features"):
        spec_raw = (FIXTURES / f"{name}.spec.json").read_bytes()
        request_raw = (FIXTURES / f"{name}.request.json").read_bytes()
        spec = json.loads(spec_raw)
        oracle_path = FIXTURES / f"{name}.oracle.json"
        result = {
            "request_sha256": hashlib.sha256(request_raw).hexdigest(),
            "spec_sha256": hashlib.sha256(spec_raw).hexdigest(),
        }
        report["workloads"][name] = result
        if not oracle_path.exists():
            result.update(reference_failure=(OUTPUT / f"{name}.failure.txt").read_text(), promotable=False)
            continue
        oracle = json.loads(oracle_path.read_bytes())
        with tempfile.TemporaryDirectory(prefix="marklab-moe-") as temporary:
            input_path = Path(temporary) / "input.csv"
            input_path.write_text(csv_text(spec))
            wire = native_wire(spec)
            for phase in ("warm", "cold"):
                samples = []
                try:
                    worker.run(request_raw)
                    for repeat in range(10):
                        pair = {}
                        order = ("python", "rust") if repeat % 2 == 0 else ("rust", "python")
                        for implementation in order:
                            if phase == "warm":
                                if implementation == "python":
                                    start = time.perf_counter_ns()
                                    value = worker.run(request_raw)
                                    json.dumps(value, separators=(",", ":"), allow_nan=False).encode()
                                    elapsed = time.perf_counter_ns() - start
                                else:
                                    packet = json.loads(
                                        subprocess.run(
                                            [str(warm), "2"],
                                            input=wire,
                                            capture_output=True,
                                            check=True,
                                            env=environment,
                                            timeout=125,
                                        ).stdout
                                    )
                                    value = packet["result"]
                                    elapsed = packet["nanoseconds"][1]
                            else:
                                output_path = Path(temporary) / f"{repeat}-{implementation}.json"
                                binary = baseline if implementation == "python" else candidate
                                command = [
                                    str(binary),
                                    "bayes",
                                    "mixture-of-experts-fusion",
                                    "--input",
                                    str(input_path),
                                    "--l2-penalty",
                                    repr(spec["l2_penalty"]),
                                    "--entropy-regularization",
                                    repr(spec["entropy_regularization"]),
                                    "--ood-validation-quantile",
                                    repr(spec["ood_validation_quantile"]),
                                    "--timeout-seconds",
                                    str(spec["timeout_seconds"]),
                                    "--out",
                                    str(output_path),
                                ]
                                start = time.perf_counter_ns()
                                subprocess.run(
                                    command,
                                    capture_output=True,
                                    check=True,
                                    env=environment,
                                    timeout=125,
                                )
                                elapsed = time.perf_counter_ns() - start
                                value = json.loads(output_path.read_bytes())
                            verify(oracle, value, name)
                            pair[implementation] = elapsed
                            if implementation == "rust":
                                result["native_backend"] = value["backend"]
                                result["native_request_sha256"] = value["request_sha256"]
                        samples.append(pair)
                    result[phase] = _summary(samples)
                    print(name, phase, result[phase]["median_paired_speedup"], flush=True)
                    if phase == "cold":
                        result["rss_time_l"] = {}
                        for label, binary in (("python", baseline), ("rust", candidate)):
                            memory_command = list(command)
                            memory_command[0] = str(binary)
                            memory_output = Path(temporary) / f"rss-{label}.json"
                            memory_command[-1] = str(memory_output)
                            measured = subprocess.run(
                                ["/usr/bin/time", "-l", *memory_command],
                                capture_output=True,
                                check=True,
                                env=environment,
                                timeout=125,
                            )
                            verify(oracle, json.loads(memory_output.read_bytes()), name)
                            result["rss_time_l"][label] = measured.stderr.decode()
                except Exception as error:
                    result[phase] = {
                        "samples_ns": samples,
                        "failure": str(error),
                        "stderr": (getattr(error, "stderr", None) or b"").decode(errors="replace"),
                        "promotable": False,
                    }
                    print(name, phase, "FAILED", error, flush=True)
        (OUTPUT / "comparison.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")


def audit_fit(candidate: Path, name: str) -> None:
    spec = json.loads((FIXTURES / f"{name}.spec.json").read_bytes())
    request_raw = (FIXTURES / f"{name}.request.json").read_bytes()
    reference = json.loads((FIXTURES / f"{name}.oracle.json").read_bytes())
    environment = os.environ.copy()
    environment.update(
        MARKLAB_DISABLE_EXTERNAL_BACKEND_EXECUTION="1",
        MARKLAB_PYTHON="/nonexistent/marklab-python",
        MARKLAB_RUNTIME_ROOT="/nonexistent/marklab-runtime",
    )
    with tempfile.TemporaryDirectory(prefix="marklab-moe-audit-") as temporary:
        input_path = Path(temporary) / "input.csv"
        output_path = Path(temporary) / "output.json"
        input_path.write_text(csv_text(spec))
        subprocess.run(
            [
                str(candidate),
                "bayes",
                "mixture-of-experts-fusion",
                "--input",
                str(input_path),
                "--l2-penalty",
                repr(spec["l2_penalty"]),
                "--entropy-regularization",
                repr(spec["entropy_regularization"]),
                "--ood-validation-quantile",
                repr(spec["ood_validation_quantile"]),
                "--timeout-seconds",
                str(spec["timeout_seconds"]),
                "--out",
                str(output_path),
            ],
            capture_output=True,
            check=True,
            env=environment,
            timeout=125,
        )
        native = json.loads(output_path.read_bytes())

    train = [row for row in spec["patients"] if row["split"] == "gate_train"]
    mean = np.asarray(reference["model"]["context_training_mean"])
    sd = np.asarray(reference["model"]["context_training_population_sd"])
    experts = len(spec["expert_names"])
    columns = 1 + len(mean) + experts

    def objective_gradient(flat: np.ndarray) -> tuple[float, np.ndarray]:
        coefficients = flat.reshape(experts, columns)
        gradient = np.zeros_like(coefficients)
        value = 0.0
        for row in train:
            context = (np.asarray(row["context"]) - mean) / sd
            available = np.asarray([item is not None for item in row["expert_probabilities"]])
            features = np.concatenate([context, available.astype(np.float64)])
            logits = coefficients[:, 0] + coefficients[:, 1:] @ features
            logits = np.where(available, logits, -np.inf)
            maximum = np.max(logits)
            weights = np.where(available, np.exp(logits - maximum), 0.0)
            weights /= np.sum(weights)
            expert_probabilities = np.asarray(
                [0.0 if item is None else item for item in row["expert_probabilities"]]
            )
            mixture = float(np.dot(weights, expert_probabilities))
            clipped = float(np.clip(mixture, 1e-12, 1.0 - 1e-12))
            label = row["label"]
            value -= label * math.log(clipped) + (1 - label) * math.log(1.0 - clipped)
            positive = weights > 0.0
            weight_log_weight = float(np.sum(weights[positive] * np.log(weights[positive])))
            value += spec["entropy_regularization"] * weight_log_weight
            loss_derivative = (
                (mixture - label) / (mixture * (1.0 - mixture))
                if 1e-12 < mixture < 1.0 - 1e-12
                else 0.0
            )
            for expert in range(experts):
                if weights[expert] == 0.0:
                    continue
                derivative = (
                    loss_derivative * weights[expert] * (expert_probabilities[expert] - mixture)
                    + spec["entropy_regularization"]
                    * weights[expert]
                    * (math.log(weights[expert]) - weight_log_weight)
                )
                gradient[expert, 0] += derivative
                gradient[expert, 1:] += derivative * features
        value += 0.5 * spec["l2_penalty"] * float(np.dot(flat, flat))
        gradient += spec["l2_penalty"] * coefficients
        return value, gradient.ravel()

    def evaluate(label: str, result: dict) -> dict:
        parameters = np.asarray(result["model"]["coefficients_row_major"])
        value, gradient = objective_gradient(parameters)
        central = np.empty_like(parameters)
        step = 1e-5
        for index in range(len(parameters)):
            upper = parameters.copy()
            lower = parameters.copy()
            upper[index] += step
            lower[index] -= step
            central[index] = (objective_gradient(upper)[0] - objective_gradient(lower)[0]) / (2 * step)
        return {
            "label": label,
            "objective": value,
            "gradient_maximum_absolute": float(np.max(np.abs(gradient))),
            "gradient_l2": float(np.linalg.norm(gradient)),
            "analytic_vs_central_maximum_absolute": float(np.max(np.abs(gradient - central))),
        }

    analytic_lbfgs = scipy.optimize.minimize(
        lambda parameters: objective_gradient(parameters),
        np.zeros(experts * columns),
        method="L-BFGS-B",
        jac=True,
        options={"maxiter": 2_000, "ftol": 1e-12, "gtol": 1e-7, "maxls": 50},
    )
    analytic_result = {
        "model": {
            "coefficients_row_major": analytic_lbfgs.x.tolist(),
        }
    }

    calls = []
    original_minimize = scipy.optimize.minimize
    worker = reference_module()

    def capture_minimize(*args, **kwargs):
        fitted = original_minimize(*args, **kwargs)
        calls.append(
            {
                "method": kwargs.get("method"),
                "success": bool(fitted.success),
                "message": str(fitted.message),
                "iterations": int(fitted.nit),
                "function_evaluations": int(fitted.nfev),
                "objective": float(fitted.fun),
                "reported_gradient_maximum_absolute": float(np.max(np.abs(fitted.jac))),
            }
        )
        return fitted

    worker.minimize = capture_minimize
    replay = worker.run(request_raw)
    verify(reference, replay, name)
    prediction_differences = []
    for expected, actual in zip(reference["predictions"], native["predictions"]):
        prediction_differences.append(
            {
                "patient_id": expected["patient_id"],
                "gate_maximum_absolute": max(
                    abs(left - right)
                    for left, right in zip(expected["gating_weights"], actual["gating_weights"])
                ),
                "raw_probability_absolute": abs(
                    expected["raw_probability"] - actual["raw_probability"]
                ),
                "calibrated_probability_absolute": abs(
                    expected["probability"] - actual["probability"]
                ),
            }
        )
    audit = {
        "workload": name,
        "reference": evaluate("frozen_scipy", reference),
        "native": evaluate("native_bounded_lbfgs", native),
        "analytic_lbfgs": {
            **evaluate("scipy_lbfgs_analytic_gradient", analytic_result),
            "success": bool(analytic_lbfgs.success),
            "message": str(analytic_lbfgs.message),
            "iterations": int(analytic_lbfgs.nit),
            "coefficient_maximum_absolute_difference_from_reference": float(
                np.max(
                    np.abs(
                        analytic_lbfgs.x
                        - np.asarray(reference["model"]["coefficients_row_major"])
                    )
                )
            ),
            "coefficient_maximum_absolute_difference_from_native": float(
                np.max(
                    np.abs(
                        analytic_lbfgs.x
                        - np.asarray(native["model"]["coefficients_row_major"])
                    )
                )
            ),
        },
        "reference_optimizer_calls": calls,
        "maximum_test_gate_absolute": max(row["gate_maximum_absolute"] for row in prediction_differences),
        "maximum_test_raw_probability_absolute": max(
            row["raw_probability_absolute"] for row in prediction_differences
        ),
        "maximum_test_calibrated_probability_absolute": max(
            row["calibrated_probability_absolute"] for row in prediction_differences
        ),
        "prediction_differences": prediction_differences,
    }
    OUTPUT.mkdir(parents=True, exist_ok=True)
    (OUTPUT / f"{name}-fit-audit.json").write_text(
        json.dumps(audit, indent=2, allow_nan=False) + "\n"
    )
    print(json.dumps({key: value for key, value in audit.items() if key != "prediction_differences"}, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--prepare", action="store_true")
    parser.add_argument("--warm", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--audit-fit", type=Path)
    parser.add_argument("--audit-case", default="representative")
    arguments = parser.parse_args()
    if arguments.prepare:
        prepare()
    if arguments.warm and arguments.baseline and arguments.candidate:
        measure(arguments.warm.resolve(), arguments.baseline.resolve(), arguments.candidate.resolve())
    if arguments.audit_fit:
        audit_fit(arguments.audit_fit.resolve(), arguments.audit_case)
