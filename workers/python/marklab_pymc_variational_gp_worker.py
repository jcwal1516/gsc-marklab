#!/usr/bin/env python3
"""Static PyMC worker for Marklab's VFE inducing-point GP."""

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
            f"{path} fields differ: missing={sorted(keys-actual)}, unknown={sorted(actual-keys)}"
        )
    return value


def num(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    value = float(value)
    if not math.isfinite(value):
        raise ContractError(f"{path} must be finite")
    return value


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low},{high}]")
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request = obj(
        request,
        {"format", "version", "backend", "model", "observations", "predictions", "inference", "resources"},
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    backend = obj(request["backend"], {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"}, "backend")
    exact(backend["name"], "pymc", "backend.name")
    exact(backend["version"], PYMC_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")
    model = obj(
        request["model"],
        {
            "format", "version", "family", "approximation", "coordinate_dimension",
            "coordinate_unit", "kernel", "mean_prior_mean", "mean_prior_sd",
            "amplitude_prior_sd", "length_scale_prior_sd_um", "noise_prior_sd", "jitter",
            "inducing_initialization", "inducing_constraint", "fit_state_ceiling", "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "variational_inducing_point_matern32_gp", "model.family")
    exact(model["approximation"], "vfe_inducing_point_mean_field_advi", "model.approximation")
    integer(model["coordinate_dimension"], "model.dimension", 1, 1)
    exact(model["coordinate_unit"], "micrometre", "model.unit")
    exact(model["kernel"], "matern_3_2", "model.kernel")
    mean_prior_mean = num(model["mean_prior_mean"], "model.mean prior mean")
    mean_prior_sd = num(model["mean_prior_sd"], "model.mean prior SD")
    amplitude_prior_sd = num(model["amplitude_prior_sd"], "model.amplitude prior")
    length_prior_sd = num(model["length_scale_prior_sd_um"], "model.length prior")
    noise_prior_sd = num(model["noise_prior_sd"], "model.noise prior")
    jitter = num(model["jitter"], "model.jitter")
    if min(mean_prior_sd, amplitude_prior_sd, length_prior_sd, noise_prior_sd, jitter) <= 0.0:
        raise ContractError("prior scales and jitter must be positive")
    exact(model["inducing_initialization"], "coordinate_quantiles", "model.initialization")
    exact(model["inducing_constraint"], "strictly_ordered", "model.constraint")
    exact(model["fit_state_ceiling"], "approximate_only", "model.fit state")
    exact(model["maturity"], "experimental", "model.maturity")

    observations = request["observations"]
    if not isinstance(observations, list) or not 8 <= len(observations) <= 2_000:
        raise ContractError("observations must contain 8-2000 rows")
    observation_ids, x, y = [], [], []
    coordinate_bits: set[int] = set()
    for index, value in enumerate(observations):
        row = obj(value, {"observation_id", "x_um", "value"}, f"observations[{index}]")
        row_id = row["observation_id"]
        coordinate = num(row["x_um"], "observation coordinate")
        if not isinstance(row_id, str) or not row_id or row_id.strip() != row_id or (observation_ids and row_id <= observation_ids[-1]):
            raise ContractError("observation IDs must be exact and strictly increasing")
        bits = np.float64(coordinate).view(np.uint64).item()
        if bits in coordinate_bits:
            raise ContractError("observation coordinates must be unique")
        coordinate_bits.add(bits)
        observation_ids.append(row_id)
        x.append(coordinate)
        y.append(num(row["value"], "observation value"))
    predictions = request["predictions"]
    if not isinstance(predictions, list) or not 1 <= len(predictions) <= 2_048:
        raise ContractError("predictions must contain 1-2048 rows")
    prediction_ids, x_new = [], []
    for index, value in enumerate(predictions):
        row = obj(value, {"prediction_id", "x_um"}, f"predictions[{index}]")
        row_id = row["prediction_id"]
        if not isinstance(row_id, str) or not row_id or row_id.strip() != row_id or (prediction_ids and row_id <= prediction_ids[-1]):
            raise ContractError("prediction IDs must be exact and strictly increasing")
        prediction_ids.append(row_id)
        x_new.append(num(row["x_um"], "prediction coordinate"))

    inference = obj(request["inference"], {"inducing_points", "starts", "iterations", "learning_rate", "posterior_draws", "maximum_cross_start_prediction_rmse", "maximum_tail_relative_change", "seed"}, "inference")
    inducing_points = integer(inference["inducing_points"], "inference.inducing points", 3, 64)
    starts = integer(inference["starts"], "inference.starts", 2, 4)
    iterations = integer(inference["iterations"], "inference.iterations", 1_000, 100_000)
    learning_rate = num(inference["learning_rate"], "inference.learning rate")
    posterior_draws = integer(inference["posterior_draws"], "inference.draws", 500, 10_000)
    maximum_cross_start_prediction_rmse = num(inference["maximum_cross_start_prediction_rmse"], "inference.stability RMSE")
    maximum_tail_relative_change = num(inference["maximum_tail_relative_change"], "inference.tail change")
    seed = integer(inference["seed"], "inference.seed", 0, 2**64 - 1)
    if inducing_points >= len(x) or not 0.0 < learning_rate <= 0.1 or maximum_cross_start_prediction_rmse <= 0.0 or not 0.0 < maximum_tail_relative_change <= 1.0:
        raise ContractError("inducing count or learning rate is invalid")
    resources = obj(request["resources"], {"maximum_observations", "maximum_predictions", "maximum_inducing_points", "maximum_variational_work", "maximum_output_bytes", "timeout_seconds"}, "resources")
    max_n = integer(resources["maximum_observations"], "resources.observations", 8, 2_000)
    max_p = integer(resources["maximum_predictions"], "resources.predictions", 1, 2_048)
    max_m = integer(resources["maximum_inducing_points"], "resources.inducing", 3, 64)
    max_work = integer(resources["maximum_variational_work"], "resources.work", 1, 5_000_000_000)
    integer(resources["maximum_output_bytes"], "resources.output", 1, 1_048_576)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    work = starts * iterations * (len(x) * inducing_points**2 + inducing_points**3) + starts * posterior_draws * len(x_new) * inducing_points**2
    if len(x) > max_n or len(x_new) > max_p or inducing_points > max_m or work > max_work:
        raise ContractError("request exceeds variational resource limits")
    return {
        "mean_prior_mean": mean_prior_mean,
        "mean_prior_sd": mean_prior_sd,
        "amplitude_prior_sd": amplitude_prior_sd,
        "length_prior_sd": length_prior_sd,
        "noise_prior_sd": noise_prior_sd,
        "jitter": jitter,
        "x": np.asarray(x),
        "y": np.asarray(y),
        "prediction_ids": prediction_ids,
        "x_new": np.asarray(x_new),
        "inducing_points": inducing_points,
        "starts": starts,
        "iterations": iterations,
        "learning_rate": learning_rate,
        "posterior_draws": posterior_draws,
        "maximum_cross_start_prediction_rmse": maximum_cross_start_prediction_rmse,
        "maximum_tail_relative_change": maximum_tail_relative_change,
        "seed": seed,
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(f"marklab-pymc-vigp-v1\0{seed}\0{purpose}\0{index}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


def summary(draws: np.ndarray) -> dict[str, float]:
    return {
        "mean": float(draws.mean()),
        "sd": float(draws.std(ddof=1)),
        "interval_lower": float(np.quantile(draws, 0.025)),
        "interval_upper": float(np.quantile(draws, 0.975)),
    }


def run_start(config: dict[str, Any], start_index: int) -> dict[str, Any]:
    quantiles = np.quantile(config["x"], np.linspace(0.0, 1.0, config["inducing_points"] + 2)[1:-1])
    spacing = max(float(np.ptp(config["x"])) / (config["inducing_points"] + 1), 1e-3)
    with pm.Model():
        mean = pm.Normal("mean", config["mean_prior_mean"], config["mean_prior_sd"])
        amplitude = pm.HalfNormal("amplitude", config["amplitude_prior_sd"])
        length_scale_um = pm.HalfNormal("length_scale_um", config["length_prior_sd"])
        noise_sd = pm.HalfNormal("noise_sd", config["noise_prior_sd"])
        inducing = pm.Normal(
            "inducing_locations_um",
            mu=quantiles,
            sigma=spacing,
            shape=config["inducing_points"],
            transform=pm.distributions.transforms.ordered,
            initval=quantiles,
        )
        covariance = amplitude**2 * pm.gp.cov.Matern32(1, ls=length_scale_um)
        gp = pm.gp.MarginalApprox(
            mean_func=pm.gp.mean.Constant(mean), cov_func=covariance, approx="VFE"
        )
        gp.marginal_likelihood(
            "y",
            X=config["x"][:, None],
            Xu=inducing[:, None],
            y=config["y"],
            sigma=noise_sd,
            jitter=config["jitter"],
        )
        approximation = pm.fit(
            n=config["iterations"],
            method="advi",
            random_seed=seed_for(config["seed"], "fit", start_index),
            progressbar=False,
            obj_optimizer=pm.adam(learning_rate=config["learning_rate"]),
        )
        trace = approximation.sample(
            config["posterior_draws"],
            random_seed=seed_for(config["seed"], "draws", start_index),
        )
        gp.conditional("f_pred", Xnew=config["x_new"][:, None], jitter=config["jitter"])
        gp.conditional(
            "y_rep",
            Xnew=config["x"][:, None],
            pred_noise=True,
            jitter=config["jitter"],
        )
        predictive = pm.sample_posterior_predictive(
            trace,
            var_names=["f_pred", "y_rep"],
            random_seed=seed_for(config["seed"], "predictive", start_index),
            progressbar=False,
        )
    history = np.asarray(approximation.hist, dtype=np.float64)
    window = min(200, max(10, len(history) // 10))
    initial_elbo = float(-history[:window].mean())
    final_elbo = float(-history[-window:].mean())
    first_tail = float(-history[-window:-window // 2].mean())
    second_tail = float(-history[-window // 2 :].mean())
    tail_change = abs(second_tail - first_tail) / max(abs(first_tail), 1.0)
    arrays = {
        name: np.asarray(trace["posterior"][name].values, dtype=np.float64)
        for name in ["mean", "amplitude", "length_scale_um", "noise_sd", "inducing_locations_um"]
    }
    prediction_draws = np.asarray(predictive["posterior_predictive"]["f_pred"].values)
    replicated_draws = np.asarray(predictive["posterior_predictive"]["y_rep"].values)
    finite = bool(
        np.isfinite(history).all()
        and all(np.isfinite(value).all() for value in arrays.values())
        and np.isfinite(prediction_draws).all()
        and np.isfinite(replicated_draws).all()
    )
    return {
        "history": history,
        "initial_elbo": initial_elbo,
        "final_elbo": final_elbo,
        "tail_change": tail_change,
        "arrays": arrays,
        "prediction_draws": prediction_draws,
        "replicated_draws": replicated_draws,
        "finite": finite,
    }


def prior_predictive_finite(config: dict[str, Any]) -> bool:
    rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    for _ in range(100):
        mean = rng.normal(config["mean_prior_mean"], config["mean_prior_sd"])
        amplitude = abs(rng.normal(0.0, config["amplitude_prior_sd"]))
        length = max(abs(rng.normal(0.0, config["length_prior_sd"])), 1e-12)
        noise = abs(rng.normal(0.0, config["noise_prior_sd"]))
        scaled = math.sqrt(3.0) * np.abs(config["x"][:, None] - config["x"][None, :]) / length
        covariance = amplitude**2 * (1.0 + scaled) * np.exp(-scaled)
        covariance += (noise**2 + config["jitter"]) * np.eye(len(config["x"]))
        draw = rng.multivariate_normal(np.full(len(config["x"]), mean), covariance)
        if not np.isfinite(draw).all():
            return False
    return True


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    starts = [run_start(config, index) for index in range(config["starts"])]
    if not all(start["finite"] for start in starts):
        raise ContractError("a variational start returned a non-finite value")
    selected_index = max(range(len(starts)), key=lambda index: starts[index]["final_elbo"])
    selected = starts[selected_index]
    prediction_means = [start["prediction_draws"].mean(axis=(0, 1)) for start in starts]
    pairwise = [
        float(np.sqrt(np.mean((prediction_means[left] - prediction_means[right]) ** 2)))
        for left in range(len(starts))
        for right in range(left + 1, len(starts))
    ]
    cross_start_rmse = max(pairwise)
    arrays = selected["arrays"]
    inducing_locations = arrays["inducing_locations_um"].mean(axis=(0, 1))
    predictions = []
    for index, prediction_id in enumerate(config["prediction_ids"]):
        draws = selected["prediction_draws"][..., index]
        predictions.append(
            {"prediction_id": prediction_id, "x_um": float(config["x_new"][index]), **summary(draws)}
        )
    replicated = selected["replicated_draws"]
    replicated_means = replicated.mean(axis=-1)
    replicated_sds = replicated.std(axis=-1, ddof=1)
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in arrays.values())
        and np.isfinite(selected["prediction_draws"]).all()
        and np.isfinite(replicated).all()
    )
    approximation_pass = (
        posterior_finite
        and prior_predictive_finite(config)
        and cross_start_rmse <= config["maximum_cross_start_prediction_rmse"]
        and all(
            start["final_elbo"] > start["initial_elbo"]
            and start["tail_change"] <= config["maximum_tail_relative_change"]
            for start in starts
        )
    )
    return {
        "format": "marklab.pymc_variational_gp_worker_result",
        "version": 1,
        "backend": {
            "name": "pymc",
            "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
        "request_sha256": request_sha,
        "fit_state": "approximate_only" if approximation_pass else "nonconverged",
        "posterior": {
            name: summary(arrays[name]) for name in ["mean", "amplitude", "length_scale_um", "noise_sd"]
        },
        "inducing_locations_um": [float(value) for value in inducing_locations],
        "predictions": predictions,
        "diagnostics": {
            "starts": [
                {
                    "start_index": index,
                    "initial_elbo": start["initial_elbo"],
                    "final_elbo": start["final_elbo"],
                    "tail_relative_change": start["tail_change"],
                    "finite": start["finite"],
                }
                for index, start in enumerate(starts)
            ],
            "selected_start": selected_index,
            "all_finite": True,
            "prior_predictive_finite": prior_predictive_finite(config),
            "posterior_finite": posterior_finite,
            "cross_start_prediction_rmse": cross_start_rmse,
            "gradient_norm_available": False,
            "importance_correction": "not_run",
        },
        "posterior_predictive": {
            "observed_mean": float(config["y"].mean()),
            "replicated_mean": float(replicated_means.mean()),
            "observed_sd": float(config["y"].std(ddof=1)),
            "replicated_sd_mean": float(replicated_sds.mean()),
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
