#!/usr/bin/env python3
"""Static PyMC worker for patient-unit Dirichlet-multinomial group regression."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import pymc as pm
import pytensor.tensor as pt


PYMC_VERSION = "6.3.0"


class ContractError(ValueError):
    pass


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


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format", "version", "backend", "model", "patients", "sampling",
            "resources", "diagnostic_policy",
        },
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    exact(request["version"], 1, "request.version")
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    exact(backend["name"], "pymc", "backend.name")
    exact(backend["version"], PYMC_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    model = obj(
        request["model"],
        {
            "format", "version", "family", "reference_group", "comparison_group",
            "class_ids", "reference_class", "baseline_logit_prior", "logit_prior_sd",
            "group_effect_prior", "group_effect_prior_sd", "concentration_prior",
            "concentration_prior_sd", "likelihood", "observation_unit", "biological_unit",
            "backend_capability", "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    exact(model["version"], 1, "model.version")
    exact(model["family"], "patient_dirichlet_multinomial_group_regression", "model.family")
    exact(model["baseline_logit_prior"], "normal_with_reference_class_zero", "model.baseline")
    exact(
        model["group_effect_prior"],
        "normal_log_ratio_difference_with_reference_class_zero",
        "model.group_effect",
    )
    exact(model["concentration_prior"], "half_normal", "model.concentration")
    exact(
        model["likelihood"],
        "dirichlet_multinomial_collapsed_patient_count_vectors",
        "model.likelihood",
    )
    exact(model["observation_unit"], "patient_complete_class_count_vector", "model.unit")
    exact(model["biological_unit"], "patient", "model.biological_unit")
    class_ids = model["class_ids"]
    if (
        not isinstance(class_ids, list)
        or not 3 <= len(class_ids) <= 16
        or any(not isinstance(value, str) or not value for value in class_ids)
        or len(set(class_ids)) != len(class_ids)
        or model["reference_class"] != class_ids[-1]
    ):
        raise ContractError("class identities are invalid")
    reference_group = model["reference_group"]
    comparison_group = model["comparison_group"]
    if (
        not isinstance(reference_group, str)
        or not reference_group
        or not isinstance(comparison_group, str)
        or not comparison_group
        or reference_group == comparison_group
    ):
        raise ContractError("group identities are invalid")
    logit_sd = number(model["logit_prior_sd"], "model.logit_sd")
    group_sd = number(model["group_effect_prior_sd"], "model.group_sd")
    concentration_sd = number(model["concentration_prior_sd"], "model.concentration_sd")
    if min(logit_sd, group_sd, concentration_sd) <= 0.0:
        raise ContractError("prior scales must be positive")
    patients = request["patients"]
    if not isinstance(patients, list) or not 8 <= len(patients) <= 512:
        raise ContractError("patient count is invalid")
    patient_ids = []
    groups = []
    counts = []
    for index, raw in enumerate(patients):
        row = obj(raw, {"patient_id", "group", "counts"}, f"patients[{index}]")
        patient_id = row["patient_id"]
        group = row["group"]
        raw_counts = row["counts"]
        if (
            not isinstance(patient_id, str)
            or not patient_id
            or len(patient_id) > 128
            or group not in {reference_group, comparison_group}
            or not isinstance(raw_counts, list)
            or len(raw_counts) != len(class_ids)
        ):
            raise ContractError("patient row identity or dimensions are invalid")
        vector = [integer(value, "patient.count", 0, 10_000_000) for value in raw_counts]
        if sum(vector) <= 0:
            raise ContractError("patient count vector must be positive")
        patient_ids.append(patient_id)
        groups.append(group)
        counts.append(vector)
    if patient_ids != sorted(patient_ids) or len(set(patient_ids)) != len(patient_ids):
        raise ContractError("patient identities must be unique and sorted")
    if min(groups.count(reference_group), groups.count(comparison_group)) < 4:
        raise ContractError("each group requires four patients")
    counts_array = np.asarray(counts, dtype=np.int64)
    totals = counts_array.sum(axis=1)
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
    resources = obj(
        request["resources"],
        {
            "maximum_patients", "maximum_classes", "maximum_total_cells",
            "maximum_total_iterations", "maximum_output_bytes", "timeout_seconds",
        },
        "resources",
    )
    if len(patients) > integer(resources["maximum_patients"], "resources.patients", 8, 512):
        raise ContractError("patient limit exceeded")
    if len(class_ids) > integer(resources["maximum_classes"], "resources.classes", 3, 16):
        raise ContractError("class limit exceeded")
    if int(totals.sum()) > integer(resources["maximum_total_cells"], "resources.cells", 1, 10_000_000):
        raise ContractError("cell limit exceeded")
    if chains * (tune + draws) > integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 400_000
    ):
        raise ContractError("iteration limit exceeded")
    policy = obj(
        request["diagnostic_policy"],
        {
            "prior_predictive_draws", "maximum_r_hat", "minimum_bulk_ess", "minimum_tail_ess",
            "minimum_ebfmi", "maximum_divergences", "maximum_tree_depth_hits",
            "maximum_tree_depth",
        },
        "policy",
    )
    return {
        "class_ids": class_ids,
        "reference_group": reference_group,
        "comparison_group": comparison_group,
        "groups": groups,
        "indicator": np.asarray([group == comparison_group for group in groups], dtype=np.int64),
        "counts": counts_array,
        "totals": totals,
        "logit_sd": logit_sd,
        "group_sd": group_sd,
        "concentration_sd": concentration_sd,
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
        "prior_draws": integer(policy["prior_predictive_draws"], "policy.prior_draws", 1, 100_000),
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.rhat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, 2**63 - 1),
        "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth hits", 0, 2**63 - 1),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(
        f"marklab-pymc-dirichlet-multinomial-group-v1\0{seed}\0{purpose}\0{index}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def summary(values: np.ndarray) -> dict[str, float]:
    flat = values.reshape(-1)
    return {
        "mean": float(flat.mean()),
        "sd": float(flat.std(ddof=1)),
        "interval_lower": float(np.quantile(flat, 0.025)),
        "interval_upper": float(np.quantile(flat, 0.975)),
    }


def tree_values(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def softmax(values: np.ndarray) -> np.ndarray:
    shifted = values - values.max(axis=-1, keepdims=True)
    weights = np.exp(shifted)
    return weights / weights.sum(axis=-1, keepdims=True)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    classes = len(config["class_ids"])
    with pm.Model():
        baseline = pm.Normal("baseline_logits", 0.0, config["logit_sd"], shape=classes - 1)
        effect = pm.Normal("group_log_ratio_effects", 0.0, config["group_sd"], shape=classes - 1)
        concentration = pm.HalfNormal("concentration", config["concentration_sd"])
        zero = pt.zeros((1,))
        reference_probability = pm.math.softmax(pt.concatenate([baseline, zero]))
        comparison_probability = pm.math.softmax(pt.concatenate([baseline + effect, zero]))
        group_probability = pt.stack([reference_probability, comparison_probability])
        patient_probability = group_probability[config["indicator"]]
        pm.DirichletMultinomial(
            "counts",
            n=config["totals"],
            a=patient_probability * concentration,
            observed=config["counts"],
        )
        prior = pm.sample_prior_predictive(
            draws=config["prior_draws"], random_seed=seed_for(config["seed"], "prior")
        )
        posterior = pm.sample(
            draws=config["draws"],
            tune=config["tune"],
            chains=config["chains"],
            cores=1,
            blas_cores=1,
            random_seed=[seed_for(config["seed"], "chain", index) for index in range(config["chains"])],
            target_accept=config["target_accept"],
            nuts_sampler="pymc",
            nuts={"max_treedepth": config["maximum_tree_depth"]},
            progressbar=False,
            quiet=True,
            compute_convergence_checks=False,
        )
        predictive = pm.sample_posterior_predictive(
            posterior,
            var_names=["counts"],
            random_seed=seed_for(config["seed"], "predictive"),
            progressbar=False,
        )
    baseline_draws = np.asarray(posterior["posterior"]["baseline_logits"].values, dtype=np.float64)
    effect_draws = np.asarray(posterior["posterior"]["group_log_ratio_effects"].values, dtype=np.float64)
    concentration_draws = np.asarray(posterior["posterior"]["concentration"].values, dtype=np.float64)
    zeros = np.zeros((*baseline_draws.shape[:-1], 1), dtype=np.float64)
    reference_draws = softmax(np.concatenate([baseline_draws, zeros], axis=-1))
    comparison_draws = softmax(np.concatenate([baseline_draws + effect_draws, zeros], axis=-1))
    difference_draws = comparison_draws - reference_draws
    replicated = np.asarray(predictive["posterior_predictive"]["counts"].values, dtype=np.int64)
    monitored = ["baseline_logits", "group_log_ratio_effects", "concentration"]
    prior_finite = bool(all(np.isfinite(np.asarray(value.values)).all() for value in prior["prior"].values()))
    posterior_finite = bool(
        np.isfinite(reference_draws).all()
        and np.isfinite(comparison_draws).all()
        and np.isfinite(concentration_draws).all()
        and np.isfinite(replicated).all()
    )
    r_hat = float(tree_values(pm.stats.rhat(posterior, var_names=monitored, method="rank"), monitored).max())
    bulk = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="bulk"), monitored).min())
    tail = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="tail"), monitored).min())
    mcse_mean = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="mean"), monitored).max())
    mcse_sd = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="sd"), monitored).max())
    energy = np.asarray(posterior["sample_stats"]["energy"].values, dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(posterior["sample_stats"]["diverging"].values).sum())
    depth_hits = int(np.asarray(posterior["sample_stats"]["reached_max_treedepth"].values).sum())
    constraints = bool(
        np.all(concentration_draws > 0.0)
        and np.all(reference_draws > 0.0)
        and np.all(comparison_draws > 0.0)
        and np.allclose(reference_draws.sum(axis=-1), 1.0)
        and np.allclose(comparison_draws.sum(axis=-1), 1.0)
    )
    complete = bool(
        prior_finite
        and posterior_finite
        and constraints
        and r_hat <= config["maximum_r_hat"]
        and bulk >= config["minimum_bulk_ess"]
        and tail >= config["minimum_tail_ess"]
        and ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and depth_hits <= config["maximum_tree_depth_hits"]
    )
    posterior_classes = []
    for index, class_id in enumerate(config["class_ids"]):
        posterior_classes.append(
            {
                "class_id": class_id,
                "reference_probability": summary(reference_draws[..., index]),
                "comparison_probability": summary(comparison_draws[..., index]),
                "difference_comparison_minus_reference": summary(difference_draws[..., index]),
            }
        )
    observed_proportions = config["counts"] / config["totals"][:, None]
    replicated_proportions = replicated / config["totals"][None, None, :, None]
    reference_mask = config["indicator"] == 0
    comparison_mask = ~reference_mask
    observed_reference = observed_proportions[reference_mask].mean(axis=0)
    observed_comparison = observed_proportions[comparison_mask].mean(axis=0)
    replicated_reference = replicated_proportions[..., reference_mask, :].mean(axis=-2)
    replicated_comparison = replicated_proportions[..., comparison_mask, :].mean(axis=-2)
    observed_difference = observed_comparison - observed_reference
    replicated_difference = replicated_comparison - replicated_reference
    predictive_classes = []
    for index, class_id in enumerate(config["class_ids"]):
        predictive_classes.append(
            {
                "class_id": class_id,
                "observed_reference_mean_proportion": float(observed_reference[index]),
                "observed_comparison_mean_proportion": float(observed_comparison[index]),
                "replicated_reference_mean_proportion": float(replicated_reference[..., index].mean()),
                "replicated_comparison_mean_proportion": float(replicated_comparison[..., index].mean()),
                "probability_replicated_difference_at_least_observed": float(
                    np.mean(replicated_difference[..., index] >= observed_difference[index])
                ),
            }
        )
    return {
        "format": "marklab.pymc_dirichlet_multinomial_group_worker_result",
        "version": 1,
        "backend": {
            "name": "pymc", "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha,
        },
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"], "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            "classes": posterior_classes,
            "concentration": summary(concentration_draws),
            "overdispersion_mean": float((1.0 / (concentration_draws + 1.0)).mean()),
        },
        "diagnostics": {
            "prior_predictive_finite": prior_finite, "posterior_finite": posterior_finite,
            "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail,
            "mcse_mean": mcse_mean, "mcse_sd": mcse_sd, "minimum_ebfmi": ebfmi,
            "divergences": divergences, "max_tree_depth_hits": depth_hits,
            "constraints_valid": constraints, "identifiability_checks_passed": True,
        },
        "posterior_predictive": {"classes": predictive_classes},
    }


def main() -> None:
    if pm.__version__ != PYMC_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("PyMC or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(2 * 1024 * 1024 + 1)
    if not raw or len(raw) > 2 * 1024 * 1024:
        raise ContractError("request size is invalid")
    result = fit(
        validate(json.loads(raw), lock_sha, worker_sha),
        hashlib.sha256(raw).hexdigest(),
        lock_sha,
        worker_sha,
    )
    encoded = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":"))
    if len(encoded.encode()) > 2 * 1_048_576:
        raise ContractError("result exceeds output limit")
    sys.stdout.write(encoded)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"Marklab PyMC Dirichlet-multinomial group worker failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
