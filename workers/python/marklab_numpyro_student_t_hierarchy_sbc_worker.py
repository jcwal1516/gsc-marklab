#!/usr/bin/env python3
"""Bounded SBC for Marklab's exact NumPyro Student-t hierarchy."""

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


REQUEST_FORMAT = "marklab.numpyro_student_t_hierarchy_sbc_worker_request"
RESULT_FORMAT = "marklab.numpyro_student_t_hierarchy_sbc_worker_result"


class ContractError(ValueError):
    pass


def load_worker() -> Any:
    path = Path(__file__).with_name("marklab_numpyro_student_t_hierarchy_worker.py")
    specification = importlib.util.spec_from_file_location("marklab_numpyro_student_t", path)
    if specification is None or specification.loader is None:
        raise ContractError("cannot load the NumPyro Student-t model")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


def obj(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(f"{path} fields differ: missing={sorted(keys-actual)}, unknown={sorted(actual-keys)}")
    return value


def number(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ContractError(f"{path} must be finite numeric")
    return float(value)


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be integer in [{low},{high}]")
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request = obj(request, {"format", "version", "backend", "jax_version", "source_request_sha256", "source_request", "calibration", "resources"}, "request")
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    backend = obj(request["backend"], {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"}, "backend")
    exact(backend["name"], "numpyro", "backend.name")
    exact(backend["version"], "0.21.0", "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")
    exact(request["jax_version"], "0.11.1", "request.jax_version")
    source_digest = request["source_request_sha256"]
    if not isinstance(source_digest, str) or len(source_digest) != 64 or any(character not in "0123456789abcdef" for character in source_digest):
        raise ContractError("source request digest is invalid")
    student = load_worker()
    pymc_digest = hashlib.sha256(Path(__file__).with_name("marklab_pymc_student_t_hierarchy_worker.py").read_bytes()).hexdigest()
    config = student.load_contract().validate(request["source_request"], lock_sha, pymc_digest)
    calibration = obj(request["calibration"], {"replicates", "rank_bins", "interval_probability", "maximum_tree_depth", "minimum_rank_uniformity_p_value", "minimum_coverage", "maximum_coverage"}, "calibration")
    config.update({"replicates": integer(calibration["replicates"], "calibration.replicates", 20, 100), "rank_bins": integer(calibration["rank_bins"], "calibration.rank_bins", 2, 100), "maximum_tree_depth": integer(calibration["maximum_tree_depth"], "calibration.maximum_tree_depth", 10, 16), "minimum_rank_p": number(calibration["minimum_rank_uniformity_p_value"], "calibration.rank_p"), "minimum_coverage": number(calibration["minimum_coverage"], "calibration.minimum_coverage"), "maximum_coverage": number(calibration["maximum_coverage"], "calibration.maximum_coverage")})
    exact(number(calibration["interval_probability"], "calibration.interval"), 0.9, "calibration.interval")
    resources = obj(request["resources"], {"maximum_replicates", "maximum_simulated_observations", "maximum_total_iterations", "maximum_output_bytes", "timeout_seconds"}, "resources")
    observations = sum(map(len, config["patient_values"]))
    if config["replicates"] > integer(resources["maximum_replicates"], "resources.replicates", 1, 100) or config["replicates"] * observations > integer(resources["maximum_simulated_observations"], "resources.observations", 1, 10_000_000) or config["replicates"] * config["chains"] * (config["tune"] + config["draws"]) > integer(resources["maximum_total_iterations"], "resources.iterations", 1, 1_000_000):
        raise ContractError("SBC resources exceeded")
    config["maximum_output_bytes"] = integer(resources["maximum_output_bytes"], "resources.output", 1, 2*1_048_576)
    return config


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    digest = hashlib.sha256(f"marklab-student-t-sbc-v1\0{seed}\0{purpose}\0{replicate}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def run_sbc(config: dict[str, Any]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    student = load_worker()
    patient_index = np.concatenate([np.full(len(values), index, dtype=np.int32) for index, values in enumerate(config["patient_values"])])
    kernel = NUTS(student.model, target_accept_prob=config["target_accept"], max_tree_depth=config["maximum_tree_depth"], dense_mass=True)
    completed: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    for replicate in range(config["replicates"]):
        try:
            rng = np.random.default_rng(seed_for(config["seed"], "simulate", replicate))
            true_global = float(rng.normal(config["global_mean"], config["global_sd"]))
            true_between = float(abs(rng.normal(0.0, config["between_sd"])))
            true_observation = float(abs(rng.normal(0.0, config["observation_sd"])))
            true_df = float(2.0 + rng.exponential(1.0/config["df_rate"]))
            patient_means = true_global + true_between * rng.normal(0.0, 1.0, len(config["patient_ids"]))
            observations = patient_means[patient_index] + true_observation * rng.standard_t(true_df, patient_index.size)
            sampler = MCMC(kernel, num_warmup=config["tune"], num_samples=config["draws"], num_chains=config["chains"], chain_method="sequential", progress_bar=False)
            sampler.run(jax.random.PRNGKey(seed_for(config["seed"], "fit", replicate)), patient_index=jnp.asarray(patient_index), observations=jnp.asarray(observations), patient_count=len(config["patient_ids"]), global_mean=config["global_mean"], global_sd=config["global_sd"], between_sd=config["between_sd"], observation_sd=config["observation_sd"], df_rate=config["df_rate"], extra_fields=("diverging", "num_steps", "energy"))
            samples = {name: np.asarray(value, dtype=np.float64) for name, value in sampler.get_samples(group_by_chain=True).items()}
            degrees = 2.0 + samples["degrees_of_freedom_excess"]
            monitored = {"global_mean": samples["global_mean"], "between_patient_sd": samples["between_patient_sd"], "observation_sd": samples["observation_sd"], "degrees_of_freedom": degrees, "patient_mean": samples["patient_mean"]}
            posterior = az.from_dict({"posterior": monitored})
            names = list(monitored)
            r_hat = float(flattened(az.rhat(posterior, var_names=names, method="rank"), names).max())
            bulk = float(flattened(az.ess(posterior, var_names=names, method="bulk"), names).min())
            tail = float(flattened(az.ess(posterior, var_names=names, method="tail"), names).min())
            extra = sampler.get_extra_fields(group_by_chain=True)
            energy = np.asarray(extra["energy"], dtype=np.float64)
            ebfmi = float(np.min(np.mean(np.diff(energy, axis=1)**2, axis=1)/np.var(energy, axis=1)))
            divergences = int(np.asarray(extra["diverging"]).sum())
            depth_hits = int((np.asarray(extra["num_steps"]) >= 2**config["maximum_tree_depth"]-1).sum())
            if not (r_hat <= config["maximum_r_hat"] and bulk >= config["minimum_bulk_ess"] and tail >= config["minimum_tail_ess"] and ebfmi >= config["minimum_ebfmi"] and divergences <= config["maximum_divergences"] and depth_hits <= config["maximum_tree_depth_hits"]):
                failures.append({"replicate": replicate, "reason": f"diagnostics_failed:r_hat={r_hat:.17g},bulk={bulk:.17g},tail={tail:.17g},ebfmi={ebfmi:.17g},divergences={divergences},depth_hits={depth_hits}"})
                continue
            values = [("global_mean", samples["global_mean"].reshape(-1), true_global), ("between_patient_sd", samples["between_patient_sd"].reshape(-1), true_between), ("observation_sd", samples["observation_sd"].reshape(-1), true_observation), ("degrees_of_freedom", degrees.reshape(-1), true_df)]
            row: dict[str, Any] = {"replicate": replicate, "true_global_mean": true_global, "true_between_patient_sd": true_between, "true_observation_sd": true_observation, "true_degrees_of_freedom": true_df, "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail, "minimum_ebfmi": ebfmi, "divergences": divergences, "max_tree_depth_hits": depth_hits}
            for name, draws, truth in values:
                row[f"{name}_rank"] = int(np.sum(draws < truth))
                row[f"{name}_covered"] = bool(np.quantile(draws, 0.05) <= truth <= np.quantile(draws, 0.95))
            completed.append(row)
        except Exception as error:
            failures.append({"replicate": replicate, "reason": f"{type(error).__name__}:{str(error)[:400]}"})
    return completed, failures


def aggregate(rows: list[dict[str, Any]], name: str, draws: int, bins: int) -> dict[str, Any]:
    ranks = np.asarray([row[f"{name}_rank"] for row in rows], dtype=np.int64)
    histogram = np.zeros(bins, dtype=np.int64)
    for rank in ranks:
        histogram[min(int(rank)*bins//(draws+1), bins-1)] += 1
    return {"rank_histogram": histogram.tolist(), "rank_uniformity_p_value": float(stats.chisquare(histogram).pvalue) if rows else 0.0, "coverage_90": float(np.mean([row[f"{name}_covered"] for row in rows])) if rows else 0.0, "mean_normalized_rank": float(ranks.mean()/draws) if rows else 0.0}


def main() -> int:
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(2*1024*1024+1)
    if not raw or len(raw) > 2*1024*1024:
        raise ContractError("request size invalid")
    config = validate(json.loads(raw), lock_sha, worker_sha)
    completed, failures = run_sbc(config)
    total_draws = config["chains"]*config["draws"]
    diagnostics = {name: aggregate(completed, name, total_draws, config["rank_bins"]) for name in ["global_mean", "between_patient_sd", "observation_sd", "degrees_of_freedom"]}
    passes = not failures and all(config["minimum_rank_p"] <= value["rank_uniformity_p_value"] and config["minimum_coverage"] <= value["coverage_90"] <= config["maximum_coverage"] for value in diagnostics.values())
    result = {"format": RESULT_FORMAT, "version": 1, "backend": {"name": "numpyro", "version": numpyro.__version__, "python_version": f"{sys.version_info.major}.{sys.version_info.minor}", "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha}, "jax_version": jax.__version__, "request_sha256": hashlib.sha256(raw).hexdigest(), "fit_state": "complete" if passes else "nonconverged", "replicates": completed, "failures": failures, "diagnostics": diagnostics}
    encoded = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")).encode()
    if len(encoded) > config["maximum_output_bytes"]:
        raise ContractError("result exceeds output limit")
    sys.stdout.buffer.write(encoded)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"Marklab Student-t SBC worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
