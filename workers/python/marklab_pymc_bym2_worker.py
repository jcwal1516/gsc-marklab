#!/usr/bin/env python3
"""Static PyMC worker for Marklab's scaled-ICAR Poisson BYM2 model."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import pymc as pm

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


def vector(value: Any, length: int, path: str) -> np.ndarray:
    if not isinstance(value, list) or len(value) != length:
        raise ContractError(f"{path} must contain exactly {length} entries")
    return np.asarray([number(item, f"{path}[]") for item in value], dtype=np.float64)


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "model",
            "region_ids",
            "weights_digest_sha256",
            "scaled_icar_transform",
            "rank_deficiency",
            "components",
            "original_typical_marginal_variance",
            "scaled_typical_marginal_variance",
            "counts",
            "expected",
            "design",
            "predictor_names",
            "sampling",
            "resources",
            "diagnostic_policy",
        },
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
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
            "format",
            "version",
            "family",
            "likelihood",
            "offset",
            "reparameterization",
            "icar_scaling",
            "intercept_prior_mean",
            "intercept_prior_sd",
            "coefficient_prior_sd",
            "total_sd_prior",
            "phi_alpha",
            "phi_beta",
            "backend_capability",
            "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "poisson_bym2_disease_mapping", "model.family")
    exact(model["likelihood"], "poisson_log_link", "model.likelihood")
    exact(model["offset"], "log_expected", "model.offset")
    exact(
        model["reparameterization"],
        "sigma_times_sqrt_phi_icar_plus_sqrt_one_minus_phi_iid",
        "model.reparameterization",
    )
    exact(
        model["icar_scaling"],
        "geometric_mean_generalized_marginal_variance_one",
        "model.icar_scaling",
    )
    exact(model["backend_capability"], "nuts", "model.capability")
    exact(model["maturity"], "experimental", "model.maturity")
    intercept_prior_mean = number(model["intercept_prior_mean"], "model.intercept mean")
    intercept_prior_sd = number(model["intercept_prior_sd"], "model.intercept sd")
    coefficient_prior_sd = number(model["coefficient_prior_sd"], "model.coefficient sd")
    total_sd_prior = number(model["total_sd_prior"], "model.total sd")
    phi_alpha = number(model["phi_alpha"], "model.phi alpha")
    phi_beta = number(model["phi_beta"], "model.phi beta")
    if min(intercept_prior_sd, coefficient_prior_sd, total_sd_prior, phi_alpha, phi_beta) <= 0.0:
        raise ContractError("BYM2 prior parameters must be positive")

    region_ids = request["region_ids"]
    if not isinstance(region_ids, list) or not 6 <= len(region_ids) <= 64:
        raise ContractError("BYM2 requires 6-64 region IDs")
    if any(
        not isinstance(value, str)
        or not value
        or value.strip() != value
        or (index > 0 and region_ids[index - 1] >= value)
        for index, value in enumerate(region_ids)
    ):
        raise ContractError("region IDs must be exact and increasing")
    dimension = len(region_ids)
    rank_deficiency = integer(request["rank_deficiency"], "rank deficiency", 1, dimension - 1)
    constrained_dimension = dimension - rank_deficiency
    components = request["components"]
    if not isinstance(components, list) or len(components) != rank_deficiency:
        raise ContractError("component count must equal rank deficiency")
    parsed_components: list[list[int]] = []
    covered: set[int] = set()
    for component_index, component in enumerate(components):
        if not isinstance(component, list) or len(component) < 2:
            raise ContractError("each component needs at least two regions")
        parsed = [
            integer(value, f"components[{component_index}][]", 0, dimension - 1)
            for value in component
        ]
        if len(set(parsed)) != len(parsed) or any(value in covered for value in parsed):
            raise ContractError("components overlap or repeat regions")
        covered.update(parsed)
        parsed_components.append(parsed)
    if covered != set(range(dimension)):
        raise ContractError("components must cover all regions")
    transform = vector(
        request["scaled_icar_transform"],
        dimension * constrained_dimension,
        "scaled_icar_transform",
    ).reshape(dimension, constrained_dimension)
    if any(abs(transform[component, :].sum(axis=0)).max() > 1e-10 for component in parsed_components):
        raise ContractError("scaled ICAR transform violates sum-to-zero")
    original_variance = number(
        request["original_typical_marginal_variance"], "original typical variance"
    )
    scaled_variance = number(
        request["scaled_typical_marginal_variance"], "scaled typical variance"
    )
    computed_scaled_variance = float(
        np.exp(np.log(np.square(transform).sum(axis=1)).mean())
    )
    if original_variance <= 0.0 or abs(scaled_variance - 1.0) > 1e-12:
        raise ContractError("declared ICAR scaling is invalid")
    if abs(computed_scaled_variance - scaled_variance) > 1e-10:
        raise ContractError("scaled transform does not have unit typical marginal variance")
    digest = request["weights_digest_sha256"]
    if not isinstance(digest, str) or len(digest) != 64 or any(
        value not in "0123456789abcdef" for value in digest.lower()
    ):
        raise ContractError("weights digest is invalid")

    counts_raw = request["counts"]
    if not isinstance(counts_raw, list) or len(counts_raw) != dimension:
        raise ContractError("count vector length mismatch")
    counts = np.asarray(
        [integer(value, "counts[]", 0, 2**63 - 1) for value in counts_raw], dtype=np.int64
    )
    expected = vector(request["expected"], dimension, "expected")
    if np.any(expected <= 0.0):
        raise ContractError("expected counts must be positive")
    predictor_names = request["predictor_names"]
    if not isinstance(predictor_names, list) or not 1 <= len(predictor_names) <= 16:
        raise ContractError("predictor count must be 1-16")
    if any(
        not isinstance(value, str)
        or not value
        or value.strip() != value
        or value == "intercept"
        or value in predictor_names[:index]
        for index, value in enumerate(predictor_names)
    ):
        raise ContractError("predictor names are invalid")
    predictors = len(predictor_names)
    design = vector(request["design"], dimension * predictors, "design").reshape(
        dimension, predictors
    )
    if np.linalg.matrix_rank(np.column_stack([np.ones(dimension), design])) != predictors + 1:
        raise ContractError("design including intercept must be full rank")

    sampling = obj(
        request["sampling"],
        {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"},
        "sampling",
    )
    chains = integer(sampling["chains"], "sampling.chains", 2, 8)
    tune = integer(sampling["tune_per_chain"], "sampling.tune", 100, 100_000)
    draws = integer(sampling["draws_per_chain"], "sampling.draws", 100, 100_000)
    target_accept = number(sampling["target_accept"], "sampling.target_accept")
    if not 0.5 <= target_accept < 1.0:
        raise ContractError("target acceptance must be in [0.5,1)")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)
    resources = obj(
        request["resources"],
        {"maximum_observations", "maximum_total_iterations", "maximum_output_bytes", "timeout_seconds"},
        "resources",
    )
    if dimension > integer(resources["maximum_observations"], "resources.observations", 1, 64):
        raise ContractError("region count exceeds resource limit")
    max_iterations = integer(resources["maximum_total_iterations"], "resources.iterations", 1, 400_000)
    integer(resources["maximum_output_bytes"], "resources.output", 1, 1_048_576)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    if chains * (tune + draws) > max_iterations:
        raise ContractError("iterations exceed resource limit")
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
        "policy",
    )
    return {
        "prior_draws": integer(policy["prior_predictive_draws"], "policy.prior", 100, 10_000),
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.rhat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, chains * draws),
        "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth hits", 0, chains * draws),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
        "intercept_prior_mean": intercept_prior_mean,
        "intercept_prior_sd": intercept_prior_sd,
        "coefficient_prior_sd": coefficient_prior_sd,
        "total_sd_prior": total_sd_prior,
        "phi_alpha": phi_alpha,
        "phi_beta": phi_beta,
        "region_ids": region_ids,
        "predictor_names": predictor_names,
        "transform": transform,
        "components": parsed_components,
        "counts": counts,
        "expected": expected,
        "design": design,
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(f"marklab-pymc-bym2-v1\0{seed}\0{purpose}\0{index}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


def summary(draws: np.ndarray) -> dict[str, float]:
    return {
        "mean": float(draws.mean()),
        "sd": float(draws.std(ddof=1)),
        "interval_lower": float(np.quantile(draws, 0.025)),
        "interval_upper": float(np.quantile(draws, 0.975)),
    }


def stats(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    dimension = len(config["region_ids"])
    predictors = len(config["predictor_names"])
    constrained_dimension = config["transform"].shape[1]
    coords = {"region": config["region_ids"], "predictor": config["predictor_names"]}
    with pm.Model(coords=coords):
        intercept = pm.Normal(
            "intercept", config["intercept_prior_mean"], config["intercept_prior_sd"]
        )
        coefficient = pm.Normal(
            "coefficient", 0.0, config["coefficient_prior_sd"], dims="predictor"
        )
        sigma = pm.HalfNormal("sigma", config["total_sd_prior"])
        phi = pm.Beta("phi", config["phi_alpha"], config["phi_beta"])
        structured_raw = pm.Normal("structured_raw", 0.0, 1.0, shape=constrained_dimension)
        unstructured_raw = pm.Normal("unstructured_raw", 0.0, 1.0, dims="region")
        structured_unit = pm.Deterministic(
            "structured_unit", pm.math.dot(config["transform"], structured_raw), dims="region"
        )
        structured_component = pm.Deterministic(
            "structured_component", sigma * pm.math.sqrt(phi) * structured_unit, dims="region"
        )
        unstructured_component = pm.Deterministic(
            "unstructured_component",
            sigma * pm.math.sqrt(1.0 - phi) * unstructured_raw,
            dims="region",
        )
        combined_effect = pm.Deterministic(
            "combined_effect", structured_component + unstructured_component, dims="region"
        )
        relative_risk = pm.Deterministic(
            "relative_risk",
            pm.math.exp(intercept + pm.math.dot(config["design"], coefficient) + combined_effect),
            dims="region",
        )
        pm.Poisson(
            "observed_count",
            mu=config["expected"] * relative_risk,
            observed=config["counts"],
            dims="region",
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
            random_seed=[seed_for(config["seed"], "chain", i) for i in range(config["chains"])],
            target_accept=config["target_accept"],
            nuts_sampler="pymc",
            nuts={"max_treedepth": config["maximum_tree_depth"]},
            progressbar=False,
            quiet=True,
            compute_convergence_checks=False,
        )
        predictive = pm.sample_posterior_predictive(
            posterior,
            var_names=["observed_count"],
            random_seed=seed_for(config["seed"], "predictive"),
            progressbar=False,
        )

    names = ["intercept", "coefficient", "sigma", "phi", "structured_raw", "unstructured_raw"]
    derived = [
        "structured_unit",
        "structured_component",
        "unstructured_component",
        "combined_effect",
        "relative_risk",
    ]
    arrays = {
        name: np.asarray(posterior["posterior"][name].values, dtype=np.float64)
        for name in names + derived
    }
    predictive_draws = np.asarray(
        predictive["posterior_predictive"]["observed_count"].values, dtype=np.float64
    )
    prior_finite = bool(
        all(np.isfinite(prior["prior"][name].values).all() for name in names)
        and np.isfinite(prior["prior_predictive"]["observed_count"].values).all()
    )
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in arrays.values())
        and np.isfinite(predictive_draws).all()
    )
    r_hat = float(stats(pm.stats.rhat(posterior, var_names=names, method="rank"), names).max())
    bulk = float(stats(pm.stats.ess(posterior, var_names=names, method="bulk"), names).min())
    tail = float(stats(pm.stats.ess(posterior, var_names=names, method="tail"), names).min())
    mcse_mean = float(stats(pm.stats.mcse(posterior, var_names=names, method="mean"), names).max())
    mcse_sd = float(stats(pm.stats.mcse(posterior, var_names=names, method="sd"), names).max())
    energy = np.asarray(posterior["sample_stats"]["energy"].values)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(posterior["sample_stats"]["divergences"].values).sum())
    depth_hits = int(np.asarray(posterior["sample_stats"]["reached_max_treedepth"].values).sum())
    flat_structured_unit = arrays["structured_unit"].reshape(-1, dimension)
    constraints_valid = bool(
        np.all(arrays["sigma"] > 0.0)
        and np.all((arrays["phi"] > 0.0) & (arrays["phi"] < 1.0))
        and all(
            np.abs(flat_structured_unit[:, component].sum(axis=1)).max() <= 1e-8
            for component in config["components"]
        )
    )
    complete = (
        prior_finite
        and posterior_finite
        and constraints_valid
        and r_hat <= config["maximum_r_hat"]
        and bulk >= config["minimum_bulk_ess"]
        and tail >= config["minimum_tail_ess"]
        and ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and depth_hits <= config["maximum_tree_depth_hits"]
    )
    flat_coefficient = arrays["coefficient"].reshape(-1, predictors)
    flat_fields = {name: arrays[name].reshape(-1, dimension) for name in derived[1:]}
    regions = [
        {
            "region_id": region_id,
            "observed_count": int(config["counts"][index]),
            "expected": float(config["expected"][index]),
            "structured_component": summary(flat_fields["structured_component"][:, index]),
            "unstructured_component": summary(flat_fields["unstructured_component"][:, index]),
            "combined_effect": summary(flat_fields["combined_effect"][:, index]),
            "relative_risk": summary(flat_fields["relative_risk"][:, index]),
        }
        for index, region_id in enumerate(config["region_ids"])
    ]
    predictive_totals = predictive_draws.sum(axis=-1)
    predictive_zeros = (predictive_draws == 0).sum(axis=-1)
    return {
        "format": "marklab.pymc_bym2_worker_result",
        "version": 1,
        "backend": {
            "name": "pymc",
            "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            "intercept": summary(arrays["intercept"].reshape(-1)),
            "coefficients": [
                {"predictor": name, **summary(flat_coefficient[:, index])}
                for index, name in enumerate(config["predictor_names"])
            ],
            "sigma": summary(arrays["sigma"].reshape(-1)),
            "phi": summary(arrays["phi"].reshape(-1)),
        },
        "regions": regions,
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            "r_hat": r_hat,
            "ess_bulk": bulk,
            "ess_tail": tail,
            "mcse_mean": mcse_mean,
            "mcse_sd": mcse_sd,
            "minimum_ebfmi": ebfmi,
            "divergences": divergences,
            "max_tree_depth_hits": depth_hits,
            "constraints_valid": constraints_valid,
            "identifiability_checks_passed": True,
        },
        "posterior_predictive": {
            "observed_total_count": int(config["counts"].sum()),
            "replicated_total_mean": float(predictive_totals.mean()),
            "replicated_total_sd": float(predictive_totals.std(ddof=1)),
            "observed_zero_regions": int((config["counts"] == 0).sum()),
            "replicated_zero_regions_mean": float(predictive_zeros.mean()),
        },
    }


def main() -> None:
    if pm.__version__ != PYMC_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("PyMC or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(raw) > 16 * 1024 * 1024:
        raise ContractError("request exceeds 16 MiB")
    request_sha = hashlib.sha256(raw).hexdigest()
    result = fit(validate(json.loads(raw), lock_sha, worker_sha), request_sha, lock_sha, worker_sha)
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"marklab PyMC worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
