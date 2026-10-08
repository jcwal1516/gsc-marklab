#!/usr/bin/env python3
"""Bounded SBC for Marklab's repeated slide beta-binomial hierarchy."""

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


REQUEST_FORMAT = "marklab.numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker_request"
RESULT_FORMAT = "marklab.numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker_result"
MODEL_WORKER_NAME = "marklab_numpyro_beta_binomial_group_gender_slide_hierarchy_worker.py"
WORKER_IDENTITY_PREFIX = (
    b"marklab.numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker_identity.v1\0"
)


class ContractError(ValueError):
    pass


def load_module(name: str) -> Any:
    path = Path(__file__).with_name(name)
    specification = importlib.util.spec_from_file_location(name.removesuffix(".py"), path)
    if specification is None or specification.loader is None:
        raise ContractError(f"cannot load {name}")
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


def validate(request: Any, lock_sha: str, worker_sha: str) -> tuple[dict[str, Any], Any]:
    request = obj(
        request,
        {"format", "version", "backend", "jax_version", "source_request_sha256", "source_request", "calibration", "resources"},
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    backend_identity = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    exact(backend_identity["name"], "numpyro", "backend.name")
    exact(backend_identity["version"], "0.21.0", "backend.version")
    exact(backend_identity["python_version"], "3.12", "backend.python")
    exact(backend_identity["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend_identity["worker_sha256"], worker_sha, "backend.worker")
    exact(request["jax_version"], "0.11.1", "request.jax_version")
    source_sha = request["source_request_sha256"]
    if not isinstance(source_sha, str) or len(source_sha) != 64 or any(
        character not in "0123456789abcdef" for character in source_sha
    ):
        raise ContractError("source request digest is invalid")
    pymc_path = Path(__file__).with_name(
        "marklab_pymc_beta_binomial_group_gender_slide_hierarchy_worker.py"
    )
    pymc = load_module(pymc_path.name)
    config = pymc.validate(
        request["source_request"],
        lock_sha,
        hashlib.sha256(pymc_path.read_bytes()).hexdigest(),
    )
    calibration = obj(
        request["calibration"],
        {
            "replicates", "rank_bins", "interval_probability", "maximum_tree_depth",
            "minimum_rank_uniformity_p_value", "minimum_coverage", "maximum_coverage",
        },
        "calibration",
    )
    config.update({
        "replicates": integer(calibration["replicates"], "calibration.replicates", 20, 100),
        "rank_bins": integer(calibration["rank_bins"], "calibration.rank_bins", 2, 100),
        "maximum_tree_depth": integer(calibration["maximum_tree_depth"], "calibration.depth", 10, 16),
        "minimum_rank_p": number(calibration["minimum_rank_uniformity_p_value"], "calibration.rank_p"),
        "minimum_coverage": number(calibration["minimum_coverage"], "calibration.minimum_coverage"),
        "maximum_coverage": number(calibration["maximum_coverage"], "calibration.maximum_coverage"),
    })
    exact(number(calibration["interval_probability"], "calibration.interval"), 0.9, "interval")
    resources = obj(
        request["resources"],
        {"maximum_replicates", "maximum_simulated_trials", "maximum_total_iterations", "maximum_output_bytes", "timeout_seconds"},
        "resources",
    )
    total_trials = int(np.asarray(config["trials"]).sum())
    if (
        config["replicates"] > integer(resources["maximum_replicates"], "resources.replicates", 1, 100)
        or config["replicates"] * total_trials
        > integer(resources["maximum_simulated_trials"], "resources.trials", 1, 200_000_000)
        or config["replicates"] * config["chains"] * (config["tune"] + config["draws"])
        > integer(resources["maximum_total_iterations"], "resources.iterations", 1, 1_000_000)
    ):
        raise ContractError("SBC resources exceeded")
    config["maximum_output_bytes"] = integer(
        resources["maximum_output_bytes"], "resources.output", 1, 2 * 1_048_576
    )
    backend = load_module(MODEL_WORKER_NAME)
    return config, backend


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    digest = hashlib.sha256(
        f"marklab-beta-binomial-group-gender-slide-hierarchy-sbc-v1\0{seed}\0{purpose}\0{replicate}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def sigmoid(values: np.ndarray | float) -> np.ndarray | float:
    return 1.0 / (1.0 + np.exp(-values))


def run_sbc(config: dict[str, Any], backend: Any) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    trials = np.asarray(config["trials"], dtype=np.int64)
    patient_index = np.asarray(config["patient_index"], dtype=np.int64)
    group_indicator = np.asarray(config["patient_group_indicator"], dtype=np.float64)
    gender_indicator = np.asarray(config["patient_gender_indicator"], dtype=np.float64)
    patient_count = len(config["patient_ids"])
    gender_weight = float(gender_indicator.mean())
    arguments = {
        "trials": jnp.asarray(trials),
        "patient_index": jnp.asarray(patient_index),
        "patient_group_indicator": jnp.asarray(group_indicator),
        "patient_gender_indicator": jnp.asarray(gender_indicator),
        "patient_count": patient_count,
        "intercept_mean": config["intercept_mean"],
        "intercept_sd": config["intercept_sd"],
        "group_sd": config["group_sd"],
        "gender_sd": config["gender_sd"],
        "patient_sd_prior": config["patient_sd_prior"],
        "concentration_sd": config["concentration_sd"],
    }
    kernel = NUTS(
        backend.model,
        target_accept_prob=config["target_accept"],
        max_tree_depth=config["maximum_tree_depth"],
        dense_mass=True,
    )
    completed: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    for replicate in range(config["replicates"]):
        try:
            rng = np.random.default_rng(seed_for(config["seed"], "simulate", replicate))
            true_intercept = float(rng.normal(config["intercept_mean"], config["intercept_sd"]))
            true_group = float(rng.normal(0.0, config["group_sd"]))
            true_gender = float(rng.normal(0.0, config["gender_sd"]))
            true_patient_sd = float(abs(rng.normal(0.0, config["patient_sd_prior"])))
            true_effects = rng.normal(0.0, true_patient_sd, patient_count)
            true_concentration = float(abs(rng.normal(0.0, config["concentration_sd"])))
            true_patient_probabilities = sigmoid(
                true_intercept
                + group_indicator * true_group
                + gender_indicator * true_gender
                + true_effects
            )
            slide_means = true_patient_probabilities[patient_index]
            slide_probabilities = rng.beta(
                slide_means * true_concentration,
                (1.0 - slide_means) * true_concentration,
            )
            observations = rng.binomial(trials, slide_probabilities)
            true_p00 = float(sigmoid(true_intercept))
            true_p10 = float(sigmoid(true_intercept + true_group))
            true_p01 = float(sigmoid(true_intercept + true_gender))
            true_p11 = float(sigmoid(true_intercept + true_group + true_gender))
            true_difference = (
                (1.0 - gender_weight) * true_p10 + gender_weight * true_p11
                - (1.0 - gender_weight) * true_p00 - gender_weight * true_p01
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
                observed_successes=jnp.asarray(observations),
                **arguments,
                extra_fields=("diverging", "num_steps", "energy"),
            )
            samples = {
                name: np.asarray(value, dtype=np.float64)
                for name, value in sampler.get_samples(group_by_chain=True).items()
            }
            names = [
                "intercept_log_odds", "group_log_odds_effect", "gender_log_odds_effect",
                "patient_log_odds_sd", "slide_concentration", "patient_log_odds_effect",
            ]
            posterior = az.from_dict({"posterior": {name: samples[name] for name in names}})
            r_hat = float(flattened(az.rhat(posterior, var_names=names, method="rank"), names).max())
            bulk = float(flattened(az.ess(posterior, var_names=names, method="bulk"), names).min())
            tail = float(flattened(az.ess(posterior, var_names=names, method="tail"), names).min())
            extra = sampler.get_extra_fields(group_by_chain=True)
            energy = np.asarray(extra["energy"], dtype=np.float64)
            ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
            divergences = int(np.asarray(extra["diverging"]).sum())
            depth_hits = int((np.asarray(extra["num_steps"]) >= 2 ** config["maximum_tree_depth"] - 1).sum())
            if not (
                r_hat <= config["maximum_r_hat"]
                and bulk >= config["minimum_bulk_ess"]
                and tail >= config["minimum_tail_ess"]
                and ebfmi >= config["minimum_ebfmi"]
                and divergences <= config["maximum_divergences"]
                and depth_hits <= config["maximum_tree_depth_hits"]
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
            intercept = samples["intercept_log_odds"].reshape(-1)
            group = samples["group_log_odds_effect"].reshape(-1)
            gender = samples["gender_log_odds_effect"].reshape(-1)
            patient_sd = samples["patient_log_odds_sd"].reshape(-1)
            concentration = samples["slide_concentration"].reshape(-1)
            effects = samples["patient_log_odds_effect"].reshape(-1, patient_count)
            probabilities = samples["patient_probability"].reshape(-1, patient_count)
            p00 = sigmoid(intercept)
            p10 = sigmoid(intercept + group)
            p01 = sigmoid(intercept + gender)
            p11 = sigmoid(intercept + group + gender)
            difference = (
                (1.0 - gender_weight) * p10 + gender_weight * p11
                - (1.0 - gender_weight) * p00 - gender_weight * p01
            )
            values = [
                ("intercept_log_odds", intercept, true_intercept),
                ("group_log_odds_effect", group, true_group),
                ("gender_log_odds_effect", gender, true_gender),
                ("patient_log_odds_sd", patient_sd, true_patient_sd),
                ("slide_concentration", concentration, true_concentration),
                ("marginal_probability_difference", difference, true_difference),
                ("patient_probability_0", probabilities[:, 0], float(true_patient_probabilities[0])),
                ("patient_random_effect_0", effects[:, 0], float(true_effects[0])),
            ]
            row: dict[str, Any] = {
                "replicate": replicate,
                "true_intercept_log_odds": true_intercept,
                "true_group_log_odds_effect": true_group,
                "true_gender_log_odds_effect": true_gender,
                "true_patient_log_odds_sd": true_patient_sd,
                "true_slide_concentration": true_concentration,
                "true_marginal_probability_difference": true_difference,
                "true_patient_probability_0": float(true_patient_probabilities[0]),
                "true_patient_random_effect_0": float(true_effects[0]),
                "r_hat": r_hat,
                "ess_bulk": bulk,
                "ess_tail": tail,
                "minimum_ebfmi": ebfmi,
                "divergences": divergences,
                "max_tree_depth_hits": depth_hits,
            }
            for name, draws, truth in values:
                row[f"{name}_rank"] = int(np.sum(draws < truth))
                row[f"{name}_covered"] = bool(
                    np.quantile(draws, 0.05) <= truth <= np.quantile(draws, 0.95)
                )
            completed.append(row)
        except Exception as error:
            failures.append({"replicate": replicate, "reason": f"{type(error).__name__}:{str(error)[:400]}"})
    return completed, failures


def aggregate(rows: list[dict[str, Any]], name: str, draws: int, bins: int) -> dict[str, Any]:
    ranks = np.asarray([row[f"{name}_rank"] for row in rows], dtype=np.int64)
    histogram = np.zeros(bins, dtype=np.int64)
    for rank in ranks:
        histogram[min(int(rank) * bins // (draws + 1), bins - 1)] += 1
    return {
        "rank_histogram": histogram.tolist(),
        "rank_uniformity_p_value": float(stats.chisquare(histogram).pvalue) if rows else 0.0,
        "coverage_90": float(np.mean([row[f"{name}_covered"] for row in rows])) if rows else 0.0,
        "mean_normalized_rank": float(ranks.mean() / draws) if rows else 0.0,
    }


def main() -> int:
    if numpyro.__version__ != "0.21.0" or jax.__version__ != "0.11.1" or sys.version_info[:2] != (3, 12):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(
        WORKER_IDENTITY_PREFIX
        + script.read_bytes()
        + b"\0"
        + script.with_name(MODEL_WORKER_NAME).read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(2 * 1024 * 1024 + 1)
    if not raw or len(raw) > 2 * 1024 * 1024:
        raise ContractError("request size is invalid")
    config, backend = validate(json.loads(raw), lock_sha, worker_sha)
    completed, failures = run_sbc(config, backend)
    total_draws = config["chains"] * config["draws"]
    diagnostics = {
        name: aggregate(completed, name, total_draws, config["rank_bins"])
        for name in [
            "intercept_log_odds", "group_log_odds_effect", "gender_log_odds_effect",
            "patient_log_odds_sd", "slide_concentration", "marginal_probability_difference",
            "patient_probability_0", "patient_random_effect_0",
        ]
    }
    passes = not failures and all(
        config["minimum_rank_p"] <= value["rank_uniformity_p_value"]
        and config["minimum_coverage"] <= value["coverage_90"] <= config["maximum_coverage"]
        for value in diagnostics.values()
    )
    result = {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "numpyro", "version": numpyro.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha,
        },
        "jax_version": jax.__version__,
        "request_sha256": hashlib.sha256(raw).hexdigest(),
        "fit_state": "complete" if passes else "nonconverged",
        "replicates": completed,
        "failures": failures,
        "diagnostics": diagnostics,
    }
    encoded = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")).encode()
    if len(encoded) > config["maximum_output_bytes"]:
        raise ContractError("result exceeds output limit")
    sys.stdout.buffer.write(encoded)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(
            f"Marklab repeated slide SBC worker failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
