#!/usr/bin/env python3
"""Bounded simulation-based calibration for Marklab's NumPyro hierarchy."""

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


REQUEST_FORMAT = "marklab.numpyro_hierarchical_sbc_worker_request"
RESULT_FORMAT = "marklab.numpyro_hierarchical_sbc_worker_result"


class ContractError(ValueError):
    pass


def load_hierarchy_worker() -> Any:
    path = Path(__file__).with_name("marklab_numpyro_hierarchical_worker.py")
    specification = importlib.util.spec_from_file_location("marklab_numpyro_hierarchy", path)
    if specification is None or specification.loader is None:
        raise ContractError("cannot load the NumPyro hierarchy model")
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
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{path} must be finite")
    return result


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low}, {high}]")
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "jax_version",
            "model",
            "sampling",
            "calibration",
            "diagnostic_policy",
            "resources",
        },
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "request.backend",
    )
    exact(backend["name"], "numpyro", "request.backend.name")
    exact(backend["version"], "0.21.0", "request.backend.version")
    exact(backend["python_version"], "3.12", "request.backend.python_version")
    exact(backend["environment_lock_sha256"], lock_digest, "request.backend.lock")
    exact(backend["worker_sha256"], worker_digest, "request.backend.worker")
    exact(request["jax_version"], "0.11.1", "request.jax_version")

    model = obj(
        request["model"],
        {
            "format",
            "version",
            "family",
            "global_mean_prior",
            "between_patient_sd_prior",
            "patient_effect_parameterization",
            "likelihood",
            "observation_unit",
            "biological_unit",
            "hierarchy",
            "spatial_component",
            "generated_quantities",
            "posterior_predictive_statistics",
            "backend_capability",
            "maturity",
        },
        "request.model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    exact(model["version"], 1, "model.version")
    exact(model["family"], "gaussian_patient_varying_intercept", "model.family")
    global_prior = obj(
        model["global_mean_prior"], {"family", "mean", "sd", "rationale"}, "global prior"
    )
    exact(global_prior["family"], "normal", "global prior family")
    global_mean = number(global_prior["mean"], "global prior mean")
    global_sd = number(global_prior["sd"], "global prior sd")
    between_prior = obj(
        model["between_patient_sd_prior"],
        {"family", "sd", "support", "rationale"},
        "between prior",
    )
    exact(between_prior["family"], "half_normal", "between prior family")
    between_sd = number(between_prior["sd"], "between prior sd")
    likelihood = obj(model["likelihood"], {"family", "known_sigma", "link"}, "likelihood")
    exact(likelihood["family"], "normal_known_sigma", "likelihood.family")
    known_sigma = number(likelihood["known_sigma"], "known sigma")
    if min(global_sd, between_sd, known_sigma) <= 0.0:
        raise ContractError("hierarchy scales must be positive")

    sampling = obj(
        request["sampling"],
        {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"},
        "sampling",
    )
    chains = integer(sampling["chains"], "sampling.chains", 2, 8)
    tune = integer(sampling["tune_per_chain"], "sampling.tune", 100, 100_000)
    draws = integer(sampling["draws_per_chain"], "sampling.draws", 100, 100_000)
    target_accept = number(sampling["target_accept"], "sampling.target_accept")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)
    if not 0.5 <= target_accept < 1.0:
        raise ContractError("target acceptance is invalid")

    calibration = obj(
        request["calibration"],
        {
            "replicates",
            "patient_count",
            "observations_per_patient",
            "rank_bins",
            "interval_probability",
            "minimum_rank_uniformity_p_value",
            "minimum_coverage",
            "maximum_coverage",
        },
        "calibration",
    )
    replicates = integer(calibration["replicates"], "calibration.replicates", 20, 100)
    patients = integer(calibration["patient_count"], "calibration.patients", 3, 32)
    observations_per_patient = integer(
        calibration["observations_per_patient"], "calibration.observations", 2, 32
    )
    rank_bins = integer(calibration["rank_bins"], "calibration.rank_bins", 2, 100)
    exact(number(calibration["interval_probability"], "calibration.interval"), 0.9, "interval")
    minimum_rank_p = number(
        calibration["minimum_rank_uniformity_p_value"], "calibration.rank p"
    )
    minimum_coverage = number(calibration["minimum_coverage"], "calibration.minimum coverage")
    maximum_coverage = number(calibration["maximum_coverage"], "calibration.maximum coverage")

    policy = obj(
        request["diagnostic_policy"],
        {
            "prior_predictive_draws",
            "maximum_r_hat",
            "minimum_bulk_ess",
            "minimum_tail_ess",
            "minimum_ebfmi",
            "maximum_divergences",
            "maximum_tree_depth_hits",
            "maximum_tree_depth",
        },
        "diagnostic_policy",
    )
    resources = obj(
        request["resources"],
        {
            "maximum_replicates",
            "maximum_simulated_observations",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    if replicates > integer(resources["maximum_replicates"], "resources.replicates", 1, 100):
        raise ContractError("replicates exceed resources")
    total_iterations = replicates * chains * (tune + draws)
    if total_iterations > integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 1_000_000
    ):
        raise ContractError("iterations exceed resources")
    if replicates * patients * observations_per_patient > integer(
        resources["maximum_simulated_observations"], "resources.observations", 1, 100_000
    ):
        raise ContractError("simulated observations exceed resources")
    return {
        "global_mean": global_mean,
        "global_sd": global_sd,
        "between_sd": between_sd,
        "known_sigma": known_sigma,
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
        "replicates": replicates,
        "patients": patients,
        "observations_per_patient": observations_per_patient,
        "rank_bins": rank_bins,
        "minimum_rank_p": minimum_rank_p,
        "minimum_coverage": minimum_coverage,
        "maximum_coverage": maximum_coverage,
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.r_hat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, 2**63 - 1),
        "maximum_tree_depth_hits": integer(
            policy["maximum_tree_depth_hits"], "policy.depth hits", 0, 2**63 - 1
        ),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
        "maximum_output_bytes": integer(
            resources["maximum_output_bytes"], "resources.output", 1, 2 * 1_048_576
        ),
    }


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    digest = hashlib.sha256(
        f"marklab-hierarchical-sbc-v1\0{seed}\0{purpose}\0{replicate}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def run_sbc(config: dict[str, Any]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    hierarchy = load_hierarchy_worker()
    patient_index = np.repeat(
        np.arange(config["patients"], dtype=np.int32), config["observations_per_patient"]
    )
    kernel = NUTS(
        hierarchy.model,
        target_accept_prob=config["target_accept"],
        max_tree_depth=config["maximum_tree_depth"],
    )
    completed = []
    failures = []
    for replicate in range(config["replicates"]):
        try:
            rng = np.random.default_rng(seed_for(config["seed"], "simulate", replicate))
            true_global = float(rng.normal(config["global_mean"], config["global_sd"]))
            true_between = float(abs(rng.normal(0.0, config["between_sd"])))
            patient_z = rng.normal(0.0, 1.0, config["patients"])
            patient_means = true_global + true_between * patient_z
            observations = rng.normal(patient_means[patient_index], config["known_sigma"])
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
                patient_index=jnp.asarray(patient_index),
                observations=jnp.asarray(observations),
                patient_count=config["patients"],
                global_prior_mean=config["global_mean"],
                global_prior_sd=config["global_sd"],
                between_prior_sd=config["between_sd"],
                known_sigma=config["known_sigma"],
                extra_fields=("diverging", "num_steps"),
            )
            samples = {
                name: np.asarray(value, dtype=np.float64)
                for name, value in sampler.get_samples(group_by_chain=True).items()
            }
            posterior = az.from_dict(
                {
                    "posterior": {
                        "global_mean": samples["global_mean"],
                        "between_patient_sd": samples["between_patient_sd"],
                    }
                }
            )
            names = ["global_mean", "between_patient_sd"]
            r_hat = float(flattened(az.rhat(posterior, var_names=names, method="rank"), names).max())
            ess_bulk = float(flattened(az.ess(posterior, var_names=names, method="bulk"), names).min())
            ess_tail = float(flattened(az.ess(posterior, var_names=names, method="tail"), names).min())
            extra = sampler.get_extra_fields(group_by_chain=True)
            divergences = int(np.asarray(extra["diverging"]).sum())
            depth_hits = int(
                (np.asarray(extra["num_steps"]) >= 2 ** config["maximum_tree_depth"] - 1).sum()
            )
            diagnostics_pass = (
                r_hat <= config["maximum_r_hat"]
                and ess_bulk >= config["minimum_bulk_ess"]
                and ess_tail >= config["minimum_tail_ess"]
                and divergences <= config["maximum_divergences"]
                and depth_hits <= config["maximum_tree_depth_hits"]
            )
            if not diagnostics_pass:
                failures.append(
                    {
                        "replicate": replicate,
                        "reason": (
                            f"diagnostics_failed:r_hat={r_hat:.17g},bulk={ess_bulk:.17g},"
                            f"tail={ess_tail:.17g},divergences={divergences},depth_hits={depth_hits}"
                        ),
                    }
                )
                continue
            global_draws = samples["global_mean"].reshape(-1)
            between_draws = samples["between_patient_sd"].reshape(-1)
            completed.append(
                {
                    "replicate": replicate,
                    "true_global_mean": true_global,
                    "true_between_patient_sd": true_between,
                    "global_mean_rank": int(np.sum(global_draws < true_global)),
                    "between_patient_sd_rank": int(np.sum(between_draws < true_between)),
                    "global_mean_covered": bool(
                        np.quantile(global_draws, 0.05) <= true_global <= np.quantile(global_draws, 0.95)
                    ),
                    "between_patient_sd_covered": bool(
                        np.quantile(between_draws, 0.05)
                        <= true_between
                        <= np.quantile(between_draws, 0.95)
                    ),
                    "r_hat": r_hat,
                    "ess_bulk": ess_bulk,
                    "ess_tail": ess_tail,
                    "divergences": divergences,
                    "max_tree_depth_hits": depth_hits,
                }
            )
        except Exception as error:
            failures.append(
                {
                    "replicate": replicate,
                    "reason": f"{type(error).__name__}:{str(error)[:400]}",
                }
            )
    return completed, failures


def aggregate(
    completed: list[dict[str, Any]], rank_name: str, coverage_name: str, draws: int, bins: int
) -> dict[str, Any]:
    ranks = np.asarray([row[rank_name] for row in completed], dtype=np.int64)
    histogram = np.zeros(bins, dtype=np.int64)
    for rank in ranks:
        histogram[min(int(rank) * bins // (draws + 1), bins - 1)] += 1
    p_value = float(stats.chisquare(histogram).pvalue) if len(completed) else 0.0
    return {
        "rank_histogram": histogram.tolist(),
        "rank_uniformity_p_value": p_value,
        "coverage_90": float(np.mean([row[coverage_name] for row in completed]))
        if completed
        else 0.0,
        "mean_normalized_rank": float(ranks.mean() / draws) if len(completed) else 0.0,
    }


def main() -> int:
    script = Path(__file__).resolve()
    lock_digest = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_digest = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(2 * 1024 * 1024 + 1)
    if not raw or len(raw) > 2 * 1024 * 1024:
        raise ContractError("request size is invalid")
    request = json.loads(raw)
    config = validate(request, lock_digest, worker_digest)
    completed, failures = run_sbc(config)
    draws = config["chains"] * config["draws"]
    diagnostics = {
        "global_mean": aggregate(
            completed, "global_mean_rank", "global_mean_covered", draws, config["rank_bins"]
        ),
        "between_patient_sd": aggregate(
            completed,
            "between_patient_sd_rank",
            "between_patient_sd_covered",
            draws,
            config["rank_bins"],
        ),
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
            "name": "numpyro",
            "version": numpyro.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_digest,
            "worker_sha256": worker_digest,
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
        print(f"Marklab hierarchy SBC worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
