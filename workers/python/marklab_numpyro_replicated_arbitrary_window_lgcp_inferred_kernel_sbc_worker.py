#!/usr/bin/env python3
"""Prior-generative SBC for the inferred-kernel replicated patient LGCP."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import math
from pathlib import Path
import sys
from typing import Any

import arviz as az
import jax
jax.config.update("jax_enable_x64", True)
import jax.numpy as jnp
import numpy as np
import numpyro
from numpyro.infer import MCMC, NUTS
from scipy import stats


REQUEST_FORMAT = "marklab.numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_sbc_request"
RESULT_FORMAT = "marklab.numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_sbc_result"
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"


class ContractError(ValueError):
    pass


def load_module(filename: str, name: str) -> Any:
    path = Path(__file__).with_name(filename)
    specification = importlib.util.spec_from_file_location(name, path)
    if specification is None or specification.loader is None:
        raise ContractError(f"cannot load {filename}")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


def obj(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys - actual)}, unknown={sorted(actual - keys)}"
        )
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def integer(value: Any, path: str, lower: int, upper: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not lower <= value <= upper:
        raise ContractError(f"{path} is outside [{lower}, {upper}]")
    return value


def number(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{path} must be finite")
    return result


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format", "version", "backend", "jax_version", "source_request_sha256",
            "source_request", "calibration", "resources",
        },
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    exact(backend["name"], "numpyro", "backend.name")
    exact(backend["version"], NUMPYRO_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python_version")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    exact(request["jax_version"], JAX_VERSION, "request.jax_version")
    source_digest = request["source_request_sha256"]
    if (
        not isinstance(source_digest, str)
        or len(source_digest) != 64
        or any(character not in "0123456789abcdef" for character in source_digest)
    ):
        raise ContractError("source request digest is invalid")
    pymc_inferred = load_module(
        "marklab_pymc_replicated_arbitrary_window_lgcp_inferred_kernel_worker.py",
        "marklab_pymc_inferred_sbc_contract",
    )
    pymc_inferred_digest = hashlib.sha256(
        Path(__file__).with_name(
            "marklab_pymc_replicated_arbitrary_window_lgcp_inferred_kernel_worker.py"
        ).read_bytes()
    ).hexdigest()
    fixed_digest = hashlib.sha256(
        Path(__file__).with_name(
            "marklab_pymc_replicated_arbitrary_window_lgcp_worker.py"
        ).read_bytes()
    ).hexdigest()
    config = pymc_inferred.validate(
        request["source_request"], lock_digest, pymc_inferred_digest, fixed_digest
    )
    calibration = obj(
        request["calibration"],
        {
            "replicates", "rank_bins", "interval_probability", "latent_node_index",
            "minimum_rank_uniformity_p_value", "minimum_coverage", "maximum_coverage",
        },
        "calibration",
    )
    config["replicates"] = integer(calibration["replicates"], "calibration.replicates", 20, 100)
    config["rank_bins"] = integer(calibration["rank_bins"], "calibration.rank_bins", 2, 20)
    config["latent_node_index"] = integer(
        calibration["latent_node_index"], "calibration.latent_node_index", 0,
        len(config["counts"]) - 1,
    )
    exact(number(calibration["interval_probability"], "calibration.interval"), 0.9, "interval")
    config["minimum_rank_p"] = number(
        calibration["minimum_rank_uniformity_p_value"], "calibration.minimum_rank_p"
    )
    config["minimum_coverage"] = number(
        calibration["minimum_coverage"], "calibration.minimum_coverage"
    )
    config["maximum_coverage"] = number(
        calibration["maximum_coverage"], "calibration.maximum_coverage"
    )
    if not (
        0.0 <= config["minimum_rank_p"] <= 1.0
        and 0.0 <= config["minimum_coverage"] <= config["maximum_coverage"] <= 1.0
    ):
        raise ContractError("calibration thresholds are invalid")
    resources = obj(
        request["resources"],
        {
            "maximum_replicates", "maximum_simulated_nodes", "maximum_total_iterations",
            "maximum_output_bytes", "timeout_seconds",
        },
        "resources",
    )
    if config["replicates"] > integer(
        resources["maximum_replicates"], "resources.maximum_replicates", 20, 100
    ):
        raise ContractError("replicate ceiling exceeded")
    if config["replicates"] * len(config["counts"]) > integer(
        resources["maximum_simulated_nodes"], "resources.maximum_simulated_nodes", 1, 51_200
    ):
        raise ContractError("simulated-node ceiling exceeded")
    iterations = config["replicates"] * config["chains"] * (config["tune"] + config["draws"])
    if iterations > integer(
        resources["maximum_total_iterations"], "resources.maximum_total_iterations", 1, 2_000_000
    ):
        raise ContractError("iteration ceiling exceeded")
    config["maximum_output_bytes"] = integer(
        resources["maximum_output_bytes"], "resources.maximum_output_bytes", 1, 2 * 1_048_576
    )
    config["source_request_sha256"] = source_digest
    return config


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    digest = hashlib.sha256(
        f"marklab-replicated-inferred-kernel-lgcp-sbc-v1\0{seed}\0{purpose}\0{replicate}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def kernel_latent(
    config: dict[str, Any], amplitude: float, length: float, rng: np.random.Generator
) -> np.ndarray:
    latent = np.empty(len(config["counts"]), dtype=np.float64)
    for pattern in range(len(config["pattern_ids"])):
        selected = np.flatnonzero(config["pattern_index"] == pattern)
        coordinates = np.asarray([
            [config["request"]["nodes"][index]["x_um"],
             config["request"]["nodes"][index]["y_um"]]
            for index in selected
        ])
        distance = np.linalg.norm(coordinates[:, None, :] - coordinates[None, :, :], axis=2)
        scaled = math.sqrt(3.0) * distance / length
        covariance = amplitude**2 * (1.0 + scaled) * np.exp(-scaled)
        covariance += config["kernel_jitter"] * np.eye(len(selected))
        block = np.linalg.cholesky(covariance) @ rng.normal(0.0, 1.0, len(selected))
        latent[selected] = block - block.mean()
    return latent


def simulate(config: dict[str, Any], replicate: int) -> tuple[dict[str, float], np.ndarray]:
    rng = np.random.default_rng(seed_for(config["seed"], "simulate", replicate))
    priors = config["priors"]
    truth: dict[str, Any] = {
        "intercept": float(rng.normal(priors["intercept_mean"], priors["intercept_sd"])),
        "group_effect": float(rng.normal(0.0, priors["group_effect_sd"])),
        "covariate_effect": float(rng.normal(0.0, priors["covariate_effect_sd"])),
        "patient_sd": float(abs(rng.normal(0.0, priors["patient_sd_scale"]))),
        "pattern_sd": float(abs(rng.normal(0.0, priors["pattern_sd_scale"]))),
        "field_amplitude": float(abs(rng.normal(0.0, config["field_amplitude_scale"]))),
        "field_length_scale_um": float(abs(rng.normal(0.0, config["field_length_scale_scale_um"]))),
    }
    if truth["field_amplitude"] <= 0.0 or truth["field_length_scale_um"] <= 0.0:
        raise ContractError("degenerate kernel prior draw")
    patient_effect = truth["patient_sd"] * rng.normal(0.0, 1.0, len(config["patient_ids"]))
    pattern_raw = rng.normal(0.0, 1.0, len(config["pattern_ids"]))
    for patient in range(len(config["patient_ids"])):
        selected = config["pattern_patient_index"] == patient
        pattern_raw[selected] -= pattern_raw[selected].mean()
    pattern_effect = truth["pattern_sd"] * pattern_raw
    latent = kernel_latent(
        config, truth["field_amplitude"], truth["field_length_scale_um"], rng
    )
    expected = config["weight"] * np.exp(
        truth["intercept"]
        + truth["group_effect"] * config["group"][config["patient_index"]]
        + truth["covariate_effect"] * config["covariate"]
        + patient_effect[config["patient_index"]]
        + pattern_effect[config["pattern_index"]]
        + latent
    )
    if not np.isfinite(expected).all() or np.any(expected <= 0.0):
        raise ContractError("nonfinite prior-generative expected count")
    counts = rng.poisson(expected).astype(np.int64)
    maximum_events = config["request"]["resources"]["maximum_total_events"]
    if int(counts.sum()) > int(maximum_events):
        raise ContractError("prior-generative event ceiling exceeded")
    truth["latent_node"] = float(latent[config["latent_node_index"]])
    return truth, counts


def run_sbc(config: dict[str, Any]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    dynamic = load_module(
        "marklab_numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_worker.py",
        "marklab_numpyro_replicated_inferred_kernel_sbc_model",
    )
    completed: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    names = [
        "intercept", "group_effect", "covariate_effect", "patient_sd", "pattern_sd",
        "field_amplitude", "field_length_scale_um", "patient_raw", "pattern_raw", "field_raw",
    ]
    for replicate in range(config["replicates"]):
        try:
            truth, counts = simulate(config, replicate)
            fit_config = dict(config)
            fit_config["counts"] = counts
            kernel = NUTS(
                dynamic.make_model(fit_config),
                target_accept_prob=config["target_accept"],
                max_tree_depth=int(config["maximum_tree_depth"]),
            )
            sampler = MCMC(
                kernel,
                num_warmup=config["tune"],
                num_samples=config["draws"],
                num_chains=config["chains"],
                chain_method="sequential",
                progress_bar=False,
            )
            sampler.run(
                jax.random.PRNGKey(seed_for(config["seed"], "fit", replicate)),
                extra_fields=("diverging", "num_steps", "energy"),
            )
            samples = {
                name: np.asarray(value, dtype=np.float64)
                for name, value in sampler.get_samples(group_by_chain=True).items()
            }
            posterior = az.from_dict({"posterior": {name: samples[name] for name in names}})
            r_hat = float(flattened(az.rhat(posterior, var_names=names, method="rank"), names).max())
            bulk = float(flattened(az.ess(posterior, var_names=names, method="bulk"), names).min())
            tail = float(flattened(az.ess(posterior, var_names=names, method="tail"), names).min())
            extra = sampler.get_extra_fields(group_by_chain=True)
            energy = np.asarray(extra["energy"], dtype=np.float64)
            ebfmi = float(np.min(
                np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)
            ))
            divergences = int(np.asarray(extra["diverging"]).sum())
            maximum_steps = 2 ** int(config["maximum_tree_depth"]) - 1
            depth_hits = int((np.asarray(extra["num_steps"]) >= maximum_steps).sum())
            policy = config["policy"]
            if not (
                r_hat <= policy["maximum_r_hat"]
                and bulk >= policy["minimum_bulk_ess"]
                and tail >= policy["minimum_tail_ess"]
                and ebfmi >= policy["minimum_ebfmi"]
                and divergences <= policy["maximum_divergences"]
                and depth_hits <= policy["maximum_tree_depth_hits"]
            ):
                failures.append({
                    "replicate": replicate,
                    "reason": (
                        f"diagnostics_failed:r_hat={r_hat:.17g},bulk={bulk:.17g},"
                        f"tail={tail:.17g},ebfmi={ebfmi:.17g},"
                        f"divergences={divergences},depth_hits={depth_hits}"
                    ),
                })
                continue
            draws = {
                "group_effect": samples["group_effect"].reshape(-1),
                "patient_sd": samples["patient_sd"].reshape(-1),
                "pattern_sd": samples["pattern_sd"].reshape(-1),
                "field_amplitude": samples["field_amplitude"].reshape(-1),
                "field_length_scale_um": samples["field_length_scale_um"].reshape(-1),
                "latent_node": samples["latent_effect"][..., config["latent_node_index"]].reshape(-1),
            }
            row: dict[str, Any] = {
                "replicate": replicate,
                "simulated_total_events": int(counts.sum()),
                "r_hat": r_hat,
                "ess_bulk": bulk,
                "ess_tail": tail,
                "minimum_ebfmi": ebfmi,
                "divergences": divergences,
                "max_tree_depth_hits": depth_hits,
            }
            for parameter, values in draws.items():
                row[f"true_{parameter}"] = truth[parameter]
                row[f"{parameter}_rank"] = int(np.sum(values < truth[parameter]))
                row[f"{parameter}_covered"] = bool(
                    np.quantile(values, 0.05) <= truth[parameter] <= np.quantile(values, 0.95)
                )
            completed.append(row)
        except Exception as error:
            failures.append(
                {"replicate": replicate, "reason": f"{type(error).__name__}:{str(error)[:400]}"}
            )
    return completed, failures


def aggregate(
    completed: list[dict[str, Any]], parameter: str, draws: int, bins: int
) -> dict[str, Any]:
    ranks = np.asarray([row[f"{parameter}_rank"] for row in completed], dtype=np.int64)
    histogram = np.zeros(bins, dtype=np.int64)
    for value in ranks:
        histogram[min(int(value) * bins // (draws + 1), bins - 1)] += 1
    return {
        "rank_histogram": histogram.tolist(),
        "rank_uniformity_p_value": float(stats.chisquare(histogram).pvalue)
        if completed else 0.0,
        "coverage_90": float(np.mean([row[f"{parameter}_covered"] for row in completed]))
        if completed else 0.0,
        "mean_normalized_rank": float(ranks.mean() / draws) if completed else 0.0,
    }


def main() -> int:
    if (
        numpyro.__version__ != NUMPYRO_VERSION
        or jax.__version__ != JAX_VERSION
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock_digest = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_digest = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if not raw or len(raw) > 16 * 1024 * 1024:
        raise ContractError("request size is invalid")
    request_sha256 = hashlib.sha256(raw).hexdigest()
    config = validate(json.loads(raw), lock_digest, worker_digest)
    completed, failures = run_sbc(config)
    draws = config["chains"] * config["draws"]
    parameters = (
        "group_effect", "patient_sd", "pattern_sd", "field_amplitude",
        "field_length_scale_um", "latent_node",
    )
    diagnostics = {
        parameter: aggregate(completed, parameter, draws, config["rank_bins"])
        for parameter in parameters
    }
    passes = not failures and all(
        config["minimum_rank_p"] <= value["rank_uniformity_p_value"]
        and config["minimum_coverage"] <= value["coverage_90"] <= config["maximum_coverage"]
        for value in diagnostics.values()
    )
    node = config["request"]["nodes"][config["latent_node_index"]]
    result = {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "numpyro", "version": numpyro.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_digest, "worker_sha256": worker_digest,
        },
        "jax_version": jax.__version__,
        "input_sha256": config["request"]["input_sha256"],
        "request_sha256": request_sha256,
        "source_request_sha256": config["source_request_sha256"],
        "fit_state": "complete" if passes else "nonconverged",
        "replicates": completed,
        "failures": failures,
        "diagnostics": diagnostics,
        "physical_latent_pattern_id": node["pattern_id"],
        "physical_latent_node_id": node["node_id"],
    }
    output = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")) + "\n"
    if len(output.encode()) > config["maximum_output_bytes"]:
        raise ContractError("SBC result exceeds output ceiling")
    sys.stdout.write(output)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(
            f"marklab inferred-kernel replicated LGCP SBC failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
