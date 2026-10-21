#!/usr/bin/env python3
"""SciPy/PyTensor worker for a Laplace approximation of a Poisson log-rate model."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

# Configure PyTensor before it is imported.
from marklab_worker_support import pytensor_config  # noqa: F401

# isort: split
import numpy as np
import pytensor
import pytensor.tensor as pt
import scipy
from scipy import optimize, special

from marklab_worker_support import versions
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
    exact,
    integer,
    number,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)

BACKEND_VERSION = f"scipy-{versions.SCIPY}+pytensor-{versions.PYTENSOR}"
NEWTON_STEPS = 32
PREDICTIVE_DRAWS = 5_000


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "model",
            "observations",
            "optimizer",
            "resources",
            "seed",
        },
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    validate_backend(
        request["backend"],
        "backend",
        name="scipy_pytensor",
        version=BACKEND_VERSION,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    model = obj(
        request["model"],
        {
            "format",
            "version",
            "family",
            "parameter",
            "parameter_support",
            "prior_family",
            "prior_mean",
            "prior_sd",
            "likelihood",
            "link",
            "transformation",
            "backend_capability",
            "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "poisson_log_rate", "model.family")
    exact(model["parameter"], "log_rate", "model.parameter")
    exact(model["parameter_support"], "real_unconstrained", "model.support")
    exact(model["prior_family"], "normal", "model.prior")
    prior_mean = number(model["prior_mean"], "model.prior mean")
    prior_sd = number(model["prior_sd"], "model.prior sd")
    exact(model["likelihood"], "poisson_exposure", "model.likelihood")
    exact(model["link"], "log", "model.link")
    exact(model["transformation"], "rate_equals_exp_log_rate", "model.transformation")
    exact(model["backend_capability"], "laplace_approximation", "model.capability")
    exact(model["maturity"], "experimental_approximation", "model.maturity")
    if prior_sd <= 0.0:
        raise ContractError("prior SD must be positive")
    observations = request["observations"]
    if not isinstance(observations, list) or not 1 <= len(observations) <= 100_000:
        raise ContractError("observations must contain 1-100000 entries")
    ids: list[str] = []
    counts: list[int] = []
    exposures: list[float] = []
    for index, raw in enumerate(observations):
        row = obj(raw, {"observation_id", "count", "exposure"}, f"observations[{index}]")
        observation_id = row["observation_id"]
        if (
            not isinstance(observation_id, str)
            or not observation_id
            or observation_id.strip() != observation_id
            or (ids and observation_id <= ids[-1])
        ):
            raise ContractError("observation IDs must be exact and increasing")
        ids.append(observation_id)
        counts.append(integer(row["count"], "observation.count", 0, 2**63 - 1))
        exposure = number(row["exposure"], "observation.exposure")
        if exposure <= 0.0:
            raise ContractError("exposures must be positive")
        exposures.append(exposure)
    optimizer = obj(
        request["optimizer"],
        {"initial_log_rate", "max_iterations", "gradient_tolerance", "method"},
        "optimizer",
    )
    initial = number(optimizer["initial_log_rate"], "optimizer.initial")
    max_iterations = integer(optimizer["max_iterations"], "optimizer.iterations", 1, 100_000)
    tolerance = number(optimizer["gradient_tolerance"], "optimizer.tolerance")
    if not 0.0 < tolerance <= 1e-2:
        raise ContractError("gradient tolerance must be in (0,1e-2]")
    exact(
        optimizer["method"],
        "scipy_bfgs_with_exact_pytensor_gradient_newton_refinement",
        "optimizer.method",
    )
    resources = obj(
        request["resources"],
        {
            "maximum_observations",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    if len(observations) > integer(
        resources["maximum_observations"], "resources.observations", 1, 100_000
    ):
        raise ContractError("observation count exceeds resource limit")
    if max_iterations > integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 100_000
    ):
        raise ContractError("iterations exceed resource limit")
    integer(resources["maximum_output_bytes"], "resources.output", 1, 1_048_576)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    seed = integer(request["seed"], "request.seed", 0, 2**64 - 1)
    return {
        "prior_mean": prior_mean,
        "prior_sd": prior_sd,
        "counts": np.asarray(counts, dtype=np.int64),
        "exposures": np.asarray(exposures, dtype=np.float64),
        "initial": initial,
        "max_iterations": max_iterations,
        "tolerance": tolerance,
        "seed": seed,
    }


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-laplace-v1", seed, purpose)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Laplace approximation of the Poisson log-rate posterior.

    Model: ``y_i ~ Poisson(e_i * exp(theta))`` with exposure ``e_i`` and
    ``theta ~ Normal(prior_mean, prior_sd)``. The mode is found by BFGS with the exact PyTensor
    gradient and refined by up to 32 Newton steps; the posterior is approximated by
    a normal with mean ``mode`` and variance ``-1 / curvature``; the rate has the implied
    lognormal approximation.
    """
    theta = pt.dscalar("log_rate")
    prior_z = (theta - config["prior_mean"]) / config["prior_sd"]
    prior_logp = -0.5 * prior_z**2 - math.log(config["prior_sd"]) - 0.5 * math.log(2.0 * math.pi)
    likelihood_logp = (
        config["counts"] * (theta + np.log(config["exposures"]))
        - config["exposures"] * pt.exp(theta)
        - special.gammaln(config["counts"] + 1.0)
    ).sum()
    log_joint = prior_logp + likelihood_logp
    gradient = pt.grad(log_joint, theta)
    second_derivative = pt.grad(gradient, theta)
    evaluate = pytensor.function([theta], [log_joint, gradient, second_derivative])

    evaluations = 0
    gradient_evaluations = 0

    def objective(value: np.ndarray) -> float:
        nonlocal evaluations
        evaluations += 1
        return -float(evaluate(float(value[0]))[0])

    def jacobian(value: np.ndarray) -> np.ndarray:
        nonlocal gradient_evaluations
        gradient_evaluations += 1
        return np.asarray([-float(evaluate(float(value[0]))[1])])

    optimized = optimize.minimize(
        objective,
        np.asarray([config["initial"]]),
        jac=jacobian,
        method="BFGS",
        options={"gtol": config["tolerance"], "maxiter": config["max_iterations"]},
    )
    mode = float(optimized.x[0])
    for _ in range(NEWTON_STEPS):
        _, gradient_value, curvature = [float(value) for value in evaluate(mode)]
        if abs(gradient_value) <= config["tolerance"]:
            break
        if not math.isfinite(curvature) or curvature >= 0.0:
            break
        mode -= gradient_value / curvature
    log_joint_value, gradient_value, curvature = [float(value) for value in evaluate(mode)]
    negative_hessian = -curvature
    positive_definite = math.isfinite(negative_hessian) and negative_hessian > 0.0
    variance = 1.0 / negative_hessian if positive_definite else math.nan
    standard_deviation = math.sqrt(variance) if positive_definite else math.nan
    optimizer_success = bool(optimized.success and abs(gradient_value) <= config["tolerance"])
    valid = optimizer_success and positive_definite
    z = 1.959963984540054
    log_lower = mode - z * standard_deviation
    log_upper = mode + z * standard_deviation
    rate_mean = math.exp(mode + 0.5 * variance)
    rate_variance = (math.exp(variance) - 1.0) * math.exp(2.0 * mode + variance)
    rate_sd = math.sqrt(rate_variance)
    rng = np.random.default_rng(seed_for(config["seed"], "predictive"))
    log_rate_draws = rng.normal(mode, standard_deviation, size=PREDICTIVE_DRAWS)
    rates = np.exp(log_rate_draws)
    replicated = rng.poisson(rates[:, None] * config["exposures"][None, :])
    replicated_totals = replicated.sum(axis=1)
    replicated_zeros = (replicated == 0).sum(axis=1)
    return {
        "format": "marklab.scipy_pytensor_poisson_laplace_worker_result",
        "version": 1,
        "backend": backend_identity("scipy_pytensor", BACKEND_VERSION, lock_sha, worker_sha),
        "request_sha256": request_sha,
        "fit_state": "approximate_only" if valid else "nonconverged",
        "optimizer": {
            "method": "scipy_bfgs_with_exact_pytensor_gradient_newton_refinement",
            "success": optimizer_success,
            "iterations": int(optimized.nit),
            "function_evaluations": evaluations,
            "gradient_evaluations": gradient_evaluations,
            "gradient_norm": abs(gradient_value),
            "message": str(optimized.message),
        },
        "mode": {"log_rate": mode, "log_joint": log_joint_value},
        "hessian": {
            "negative_hessian": negative_hessian,
            "variance": variance,
            "standard_deviation": standard_deviation,
            "condition_number": 1.0,
            "positive_definite": positive_definite,
        },
        "log_rate_approximation": {
            "mean": mode,
            "sd": standard_deviation,
            "interval_lower": log_lower,
            "interval_upper": log_upper,
        },
        "rate_approximation": {
            "mean": rate_mean,
            "sd": rate_sd,
            "interval_lower": math.exp(log_lower),
            "interval_upper": math.exp(log_upper),
            "transformation": "lognormal_from_gaussian_log_rate",
        },
        "posterior_predictive": {
            "observed_total_count": int(config["counts"].sum()),
            "replicated_total_mean": float(replicated_totals.mean()),
            "replicated_total_sd": float(replicated_totals.std(ddof=1)),
            "observed_zero_count": int((config["counts"] == 0).sum()),
            "replicated_zero_count_mean": float(replicated_zeros.mean()),
        },
    }


def main() -> None:
    require_versions(scipy=scipy.__version__, pytensor=pytensor.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(16 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    write_result(fit(config, sha256_hex(raw), lock_sha, worker_sha))


if __name__ == "__main__":
    run_worker(main, "marklab Laplace worker failed")
