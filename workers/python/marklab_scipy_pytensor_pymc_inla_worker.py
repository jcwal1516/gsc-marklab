#!/usr/bin/env python3
"""Static nested-Laplace and PyMC worker for a Poisson-lognormal model."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import pymc as pm
import pytensor
import pytensor.tensor as pt
import scipy
from scipy import optimize, special, stats as scipy_stats

BACKEND_VERSION = "scipy-1.18.1+pytensor-3.2.4+pymc-6.3.0"


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


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "model",
            "observations",
            "grid",
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
    exact(backend["name"], "scipy_pytensor_pymc", "backend.name")
    exact(backend["version"], BACKEND_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")

    model = obj(
        request["model"],
        {
            "format",
            "version",
            "family",
            "latent_field",
            "latent_mean",
            "hyperparameter",
            "tau_prior",
            "tau_shape",
            "tau_rate",
            "likelihood",
            "link",
            "integration_coordinate",
            "backend_capability",
            "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "poisson_lognormal_latent_gaussian", "model.family")
    exact(
        model["latent_field"],
        "conditionally_independent_normal_log_rates",
        "model.latent_field",
    )
    latent_mean = number(model["latent_mean"], "model.latent_mean")
    exact(model["hyperparameter"], "precision_tau", "model.hyperparameter")
    exact(model["tau_prior"], "gamma_shape_rate", "model.tau_prior")
    tau_shape = number(model["tau_shape"], "model.tau_shape")
    tau_rate = number(model["tau_rate"], "model.tau_rate")
    exact(model["likelihood"], "poisson_exposure", "model.likelihood")
    exact(model["link"], "log", "model.link")
    exact(model["integration_coordinate"], "log_tau_with_jacobian", "model.coordinate")
    exact(
        model["backend_capability"],
        "inla_style_nested_laplace_with_nuts_comparison",
        "model.capability",
    )
    exact(model["maturity"], "experimental_approximation", "model.maturity")
    if min(tau_shape, tau_rate) <= 0.0:
        raise ContractError("tau Gamma shape and rate must be positive")

    observations = request["observations"]
    if not isinstance(observations, list) or not 3 <= len(observations) <= 16:
        raise ContractError("observations must contain 3-16 entries")
    region_ids: list[str] = []
    counts: list[int] = []
    exposures: list[float] = []
    for index, raw in enumerate(observations):
        row = obj(raw, {"observation_id", "count", "exposure"}, f"observations[{index}]")
        region_id = row["observation_id"]
        if (
            not isinstance(region_id, str)
            or not region_id
            or region_id.strip() != region_id
            or (region_ids and region_id <= region_ids[-1])
        ):
            raise ContractError("region IDs must be exact and increasing")
        region_ids.append(region_id)
        counts.append(integer(row["count"], "observation.count", 0, 2**63 - 1))
        exposure = number(row["exposure"], "observation.exposure")
        if exposure <= 0.0:
            raise ContractError("exposures must be positive")
        exposures.append(exposure)

    grid = obj(
        request["grid"],
        {
            "log_tau_min",
            "log_tau_max",
            "points",
            "endpoint_mass_limit",
            "mode_gradient_tolerance",
            "maximum_mode_iterations",
            "hmc_mean_rmse_limit",
        },
        "grid",
    )
    log_tau_min = number(grid["log_tau_min"], "grid.log_tau_min")
    log_tau_max = number(grid["log_tau_max"], "grid.log_tau_max")
    grid_points = integer(grid["points"], "grid.points", 21, 201)
    endpoint_limit = number(grid["endpoint_mass_limit"], "grid.endpoint_limit")
    mode_tolerance = number(grid["mode_gradient_tolerance"], "grid.mode_tolerance")
    maximum_mode_iterations = integer(
        grid["maximum_mode_iterations"], "grid.mode_iterations", 1, 100_000
    )
    rmse_limit = number(grid["hmc_mean_rmse_limit"], "grid.rmse_limit")
    if (
        log_tau_min >= log_tau_max
        or not 0.0 < endpoint_limit < 0.5
        or not 0.0 < mode_tolerance <= 1e-2
        or rmse_limit <= 0.0
    ):
        raise ContractError("grid controls are invalid")

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
        raise ContractError("target acceptance must be in [0.5,1)")

    resources = obj(
        request["resources"],
        {"maximum_observations", "maximum_total_iterations", "maximum_output_bytes", "timeout_seconds"},
        "resources",
    )
    if len(observations) > integer(
        resources["maximum_observations"], "resources.observations", 1, 16
    ):
        raise ContractError("observation count exceeds resource limit")
    maximum_total = integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 400_000
    )
    if chains * (tune + draws) > maximum_total:
        raise ContractError("NUTS iterations exceed resource limit")
    integer(resources["maximum_output_bytes"], "resources.output", 1, 1_048_576)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)

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
    parsed_policy = {
        "prior_draws": integer(policy["prior_predictive_draws"], "policy.prior_draws", 1, 100_000),
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.rhat"),
        "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"),
        "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.divergences", 0, 2**63 - 1),
        "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth_hits", 0, 2**63 - 1),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
    }
    if (
        parsed_policy["maximum_r_hat"] < 1.0
        or parsed_policy["minimum_bulk_ess"] <= 0.0
        or parsed_policy["minimum_tail_ess"] <= 0.0
        or parsed_policy["minimum_ebfmi"] <= 0.0
    ):
        raise ContractError("diagnostic policy is invalid")
    return {
        "region_ids": region_ids,
        "counts": np.asarray(counts, dtype=np.int64),
        "exposures": np.asarray(exposures, dtype=np.float64),
        "latent_mean": latent_mean,
        "tau_shape": tau_shape,
        "tau_rate": tau_rate,
        "log_tau_min": log_tau_min,
        "log_tau_max": log_tau_max,
        "grid_points": grid_points,
        "endpoint_limit": endpoint_limit,
        "mode_tolerance": mode_tolerance,
        "maximum_mode_iterations": maximum_mode_iterations,
        "rmse_limit": rmse_limit,
        "chains": chains,
        "tune": tune,
        "draws": draws,
        "target_accept": target_accept,
        "seed": seed,
        **parsed_policy,
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(f"marklab-inla-v1\0{seed}\0{purpose}\0{index}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


def tree_values(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def weighted_quantile(values: np.ndarray, weights: np.ndarray, probability: float) -> float:
    cumulative = np.cumsum(weights)
    return float(np.interp(probability, cumulative, values, left=values[0], right=values[-1]))


def mixture_quantile(
    modes: np.ndarray, variances: np.ndarray, weights: np.ndarray, probability: float
) -> float:
    standard_deviations = np.sqrt(variances)

    def objective(value: float) -> float:
        return float(
            np.dot(weights, scipy_stats.norm.cdf((value - modes) / standard_deviations))
            - probability
        )

    lower = float(np.min(modes - 12.0 * standard_deviations))
    upper = float(np.max(modes + 12.0 * standard_deviations))
    return float(optimize.brentq(objective, lower, upper, xtol=1e-12))


def nested_laplace(config: dict[str, Any]) -> dict[str, Any]:
    dimension = len(config["region_ids"])
    latent = pt.dvector("latent")
    tau_symbol = pt.dscalar("tau")
    likelihood = (
        config["counts"] * latent
        - config["exposures"] * pt.exp(latent)
        - special.gammaln(config["counts"] + 1.0)
    ).sum()
    centered = latent - config["latent_mean"]
    latent_prior = (
        0.5 * dimension * pt.log(tau_symbol)
        - 0.5 * dimension * math.log(2.0 * math.pi)
        - 0.5 * tau_symbol * pt.dot(centered, centered)
    )
    log_joint = likelihood + latent_prior
    gradient = pt.grad(log_joint, latent)
    evaluate = pytensor.function([latent, tau_symbol], [log_joint, gradient])

    log_taus = np.linspace(
        config["log_tau_min"], config["log_tau_max"], config["grid_points"]
    )
    taus = np.exp(log_taus)
    modes = np.empty((config["grid_points"], dimension), dtype=np.float64)
    variances = np.empty_like(modes)
    maximum_gradients = np.empty(config["grid_points"], dtype=np.float64)
    minimum_hessians = np.empty(config["grid_points"], dtype=np.float64)
    log_densities = np.empty(config["grid_points"], dtype=np.float64)
    initial = np.log((config["counts"] + 0.5) / config["exposures"])

    for grid_index, (log_tau, tau) in enumerate(zip(log_taus, taus)):
        def objective(value: np.ndarray) -> float:
            return -float(evaluate(value, tau)[0])

        def jacobian(value: np.ndarray) -> np.ndarray:
            return -np.asarray(evaluate(value, tau)[1], dtype=np.float64)

        optimized = optimize.minimize(
            objective,
            initial,
            jac=jacobian,
            method="BFGS",
            options={
                "gtol": config["mode_tolerance"],
                "maxiter": config["maximum_mode_iterations"],
            },
        )
        mode = np.asarray(optimized.x, dtype=np.float64)
        for _ in range(64):
            _, gradient_value = evaluate(mode, tau)
            gradient_value = np.asarray(gradient_value, dtype=np.float64)
            if float(np.max(np.abs(gradient_value))) <= config["mode_tolerance"]:
                break
            negative_hessian = config["exposures"] * np.exp(mode) + tau
            mode += gradient_value / negative_hessian
        log_joint_value, gradient_value = evaluate(mode, tau)
        gradient_value = np.asarray(gradient_value, dtype=np.float64)
        negative_hessian = config["exposures"] * np.exp(mode) + tau
        mode_variance = 1.0 / negative_hessian
        tau_log_prior = (
            config["tau_shape"] * math.log(config["tau_rate"])
            - special.gammaln(config["tau_shape"])
            + (config["tau_shape"] - 1.0) * log_tau
            - config["tau_rate"] * tau
        )
        log_densities[grid_index] = (
            float(log_joint_value)
            + 0.5 * dimension * math.log(2.0 * math.pi)
            - 0.5 * float(np.log(negative_hessian).sum())
            + tau_log_prior
            + log_tau
        )
        modes[grid_index] = mode
        variances[grid_index] = mode_variance
        maximum_gradients[grid_index] = float(np.max(np.abs(gradient_value)))
        minimum_hessians[grid_index] = float(np.min(negative_hessian))
        initial = mode

    step = float(log_taus[1] - log_taus[0])
    maximum_log_density = float(log_densities.max())
    trapezoid = np.ones(config["grid_points"], dtype=np.float64)
    trapezoid[[0, -1]] = 0.5
    raw_masses = np.exp(log_densities - maximum_log_density) * trapezoid * step
    total_mass = float(raw_masses.sum())
    if not math.isfinite(total_mass) or total_mass <= 0.0:
        raise ContractError("nested-Laplace grid has no finite positive mass")
    weights = raw_masses / total_mass
    weights[-1] += 1.0 - float(weights.sum())
    log_normalizer = maximum_log_density + math.log(total_mass)

    tau_mean = float(np.dot(weights, taus))
    tau_variance = float(np.dot(weights, taus**2) - tau_mean**2)
    latent_means = weights @ modes
    latent_variances = weights @ (variances + modes**2) - latent_means**2
    latent_variances = np.maximum(latent_variances, np.finfo(np.float64).eps)
    approximated_total_mean = float(
        np.dot(config["exposures"], weights @ np.exp(modes + 0.5 * variances))
    )
    return {
        "log_taus": log_taus,
        "taus": taus,
        "modes": modes,
        "variances": variances,
        "maximum_gradients": maximum_gradients,
        "minimum_hessians": minimum_hessians,
        "log_densities": log_densities,
        "weights": weights,
        "step": step,
        "log_normalizer": log_normalizer,
        "tau_mean": tau_mean,
        "tau_sd": math.sqrt(max(tau_variance, np.finfo(np.float64).eps)),
        "tau_lower": weighted_quantile(taus, weights, 0.025),
        "tau_upper": weighted_quantile(taus, weights, 0.975),
        "latent_means": latent_means,
        "latent_sds": np.sqrt(latent_variances),
        "approximated_total_mean": approximated_total_mean,
    }


def hmc_comparison(config: dict[str, Any]) -> dict[str, Any]:
    dimension = len(config["region_ids"])
    coords = {"region": config["region_ids"]}
    with pm.Model(coords=coords):
        tau = pm.Gamma("tau", alpha=config["tau_shape"], beta=config["tau_rate"])
        latent_raw = pm.Normal("latent_raw", 0.0, 1.0, dims="region")
        latent = pm.Deterministic(
            "latent", config["latent_mean"] + latent_raw / pt.sqrt(tau), dims="region"
        )
        pm.Poisson(
            "observed_count",
            mu=config["exposures"] * pt.exp(latent),
            observed=config["counts"],
            dims="region",
        )
        posterior = pm.sample(
            draws=config["draws"],
            tune=config["tune"],
            chains=config["chains"],
            cores=1,
            blas_cores=1,
            random_seed=[
                seed_for(config["seed"], "chain", chain)
                for chain in range(config["chains"])
            ],
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

    monitored = ["tau", "latent_raw"]
    tau_draws = np.asarray(posterior["posterior"]["tau"].values, dtype=np.float64)
    latent_draws = np.asarray(posterior["posterior"]["latent"].values, dtype=np.float64)
    predictive_draws = np.asarray(
        predictive["posterior_predictive"]["observed_count"].values, dtype=np.float64
    )
    r_hat = float(tree_values(pm.stats.rhat(posterior, var_names=monitored, method="rank"), monitored).max())
    bulk = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="bulk"), monitored).min())
    tail = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="tail"), monitored).min())
    energy = np.asarray(posterior["sample_stats"]["energy"].values, dtype=np.float64)
    ebfmi = float(
        np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1))
    )
    divergences = int(np.asarray(posterior["sample_stats"]["diverging"].values).sum())
    depth_hits = int(
        np.asarray(posterior["sample_stats"]["reached_max_treedepth"].values).sum()
    )
    rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    prior_tau = rng.gamma(
        config["tau_shape"], 1.0 / config["tau_rate"], size=config["prior_draws"]
    )
    prior_latent = config["latent_mean"] + rng.normal(
        size=(config["prior_draws"], dimension)
    ) / np.sqrt(prior_tau[:, None])
    finite = bool(
        np.isfinite(prior_tau).all()
        and np.isfinite(prior_latent).all()
        and np.isfinite(tau_draws).all()
        and np.isfinite(latent_draws).all()
        and np.isfinite(predictive_draws).all()
        and np.all(tau_draws > 0.0)
    )
    complete = (
        finite
        and r_hat <= config["maximum_r_hat"]
        and bulk >= config["minimum_bulk_ess"]
        and tail >= config["minimum_tail_ess"]
        and ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and depth_hits <= config["maximum_tree_depth_hits"]
    )
    flattened_latent = latent_draws.reshape(-1, dimension)
    predictive_totals = predictive_draws.sum(axis=-1).reshape(-1)
    return {
        "fit_state": "complete" if complete else "nonconverged",
        "latent_means": flattened_latent.mean(axis=0),
        "r_hat": r_hat,
        "ess_bulk": bulk,
        "ess_tail": tail,
        "minimum_ebfmi": ebfmi,
        "divergences": divergences,
        "depth_hits": depth_hits,
        "predictive_mean": float(predictive_totals.mean()),
        "predictive_sd": float(predictive_totals.std(ddof=1)),
    }


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    approximation = nested_laplace(config)
    hmc = hmc_comparison(config)
    rmse = float(
        np.sqrt(np.mean((approximation["latent_means"] - hmc["latent_means"]) ** 2))
    )
    all_modes_converged = bool(
        np.all(approximation["maximum_gradients"] <= config["mode_tolerance"])
    )
    all_hessians_positive = bool(np.all(approximation["minimum_hessians"] > 0.0))
    valid = (
        all_modes_converged
        and all_hessians_positive
        and approximation["weights"][0] <= config["endpoint_limit"]
        and approximation["weights"][-1] <= config["endpoint_limit"]
        and hmc["fit_state"] == "complete"
        and rmse <= config["rmse_limit"]
    )
    latent_marginals = []
    for index, region_id in enumerate(config["region_ids"]):
        modes = approximation["modes"][:, index]
        variances = approximation["variances"][:, index]
        latent_marginals.append(
            {
                "region_id": region_id,
                "mean": float(approximation["latent_means"][index]),
                "sd": float(approximation["latent_sds"][index]),
                "interval_lower": mixture_quantile(
                    modes, variances, approximation["weights"], 0.025
                ),
                "interval_upper": mixture_quantile(
                    modes, variances, approximation["weights"], 0.975
                ),
                "hmc_mean": float(hmc["latent_means"][index]),
            }
        )
    return {
        "format": "marklab.scipy_pytensor_pymc_inla_worker_result",
        "version": 1,
        "backend": {
            "name": "scipy_pytensor_pymc",
            "version": BACKEND_VERSION,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
        "request_sha256": request_sha,
        "fit_state": "approximate_only" if valid else "nonconverged",
        "grid": [
            {
                "log_tau": float(approximation["log_taus"][index]),
                "tau": float(approximation["taus"][index]),
                "log_unnormalized_density": float(approximation["log_densities"][index]),
                "normalized_weight": float(approximation["weights"][index]),
                "maximum_mode_gradient": float(approximation["maximum_gradients"][index]),
                "minimum_negative_hessian": float(approximation["minimum_hessians"][index]),
            }
            for index in range(config["grid_points"])
        ],
        "grid_diagnostics": {
            "step": approximation["step"],
            "log_normalizing_constant": approximation["log_normalizer"],
            "weight_sum": float(approximation["weights"].sum()),
            "left_endpoint_mass": float(approximation["weights"][0]),
            "right_endpoint_mass": float(approximation["weights"][-1]),
            "all_modes_converged": all_modes_converged,
            "all_hessians_positive": all_hessians_positive,
        },
        "tau": {
            "mean": approximation["tau_mean"],
            "sd": approximation["tau_sd"],
            "interval_lower": approximation["tau_lower"],
            "interval_upper": approximation["tau_upper"],
        },
        "latent_marginals": latent_marginals,
        "hmc_comparison": {
            "fit_state": hmc["fit_state"],
            "latent_mean_rmse": rmse,
            "r_hat": hmc["r_hat"],
            "ess_bulk": hmc["ess_bulk"],
            "ess_tail": hmc["ess_tail"],
            "minimum_ebfmi": hmc["minimum_ebfmi"],
            "divergences": hmc["divergences"],
            "max_tree_depth_hits": hmc["depth_hits"],
        },
        "posterior_predictive": {
            "observed_total_count": int(config["counts"].sum()),
            "approximated_total_mean": approximation["approximated_total_mean"],
            "hmc_replicated_total_mean": hmc["predictive_mean"],
            "hmc_replicated_total_sd": hmc["predictive_sd"],
        },
    }


def main() -> None:
    if (
        scipy.__version__ != "1.18.1"
        or pytensor.__version__ != "3.2.4"
        or pm.__version__ != "6.3.0"
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("SciPy, PyTensor, PyMC, or Python version drift")
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
        print(f"marklab INLA worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
