#!/usr/bin/env python3

import contextlib
import hashlib
import io
import json
import math
import sys

import numpy as np
import pymc as pm
import pytensor


class ContractError(Exception):
    pass


def main():
    if (
        pm.__version__ != "6.3.0"
        or pytensor.__version__ != "3.2.4"
        or np.__version__ != "2.4.6"
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("spatial latent backend version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.pymc_spatial_latent_factor_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    rows = request["rows"]
    values = np.asarray([row["values"] for row in rows], dtype=float)
    observed = np.asarray([row["observed"] for row in rows], dtype=bool)
    coordinates = np.asarray([row["coordinates_um"] for row in rows], dtype=float)
    means = np.asarray([values[:, feature][observed[:, feature]].mean() for feature in range(values.shape[1])])
    scales = np.asarray([values[:, feature][observed[:, feature]].std(ddof=0) for feature in range(values.shape[1])])
    if not np.isfinite(scales).all() or np.any(scales <= 0.0):
        raise ContractError("every spatial latent feature must vary")
    standardized = (values - means) / scales
    observed_rows, observed_features = np.nonzero(observed)
    captured = io.StringIO()
    with contextlib.redirect_stdout(captured), contextlib.redirect_stderr(captured):
        with pm.Model():
            length_scale_um = pm.HalfNormal(
                "length_scale_um", sigma=request["length_scale_prior_um"]
            )
            covariance = pm.gp.cov.Matern32(input_dim=2, ls=length_scale_um)(coordinates)
            cholesky = pm.math.cholesky(covariance + 1e-6 * np.eye(len(rows)))
            factor_raw = pm.Normal("factor_raw", 0.0, 1.0, shape=len(rows))
            spatial_factor = pm.Deterministic("spatial_factor", pm.math.dot(cholesky, factor_raw))
            loadings = pm.Normal("loadings", 0.0, 1.0, shape=values.shape[1])
            linear_predictor = spatial_factor[:, None] * loadings[None, :]
            pm.Normal(
                "observed_values",
                mu=linear_predictor[observed_rows, observed_features],
                sigma=request["noise_standard_deviation"],
                observed=standardized[observed],
            )
            prior = pm.sample_prior_predictive(draws=32, random_seed=request["seed"])
            posterior = pm.sample(
                draws=request["samples"],
                tune=request["warmup"],
                chains=2,
                cores=1,
                target_accept=request["target_accept"],
                random_seed=[request["seed"], request["seed"] + 1],
                progressbar=False,
                compute_convergence_checks=False,
                return_inferencedata=True,
                nuts_sampler="pymc",
            )
    factors = np.asarray(posterior.posterior["spatial_factor"], dtype=float).reshape(-1, len(rows))
    loading_draws = np.asarray(posterior.posterior["loadings"], dtype=float).reshape(-1, values.shape[1])
    length_draws = np.asarray(posterior.posterior["length_scale_um"], dtype=float).reshape(-1)
    sign_flips = 0
    for draw in range(len(factors)):
        pivot = int(np.argmax(np.abs(loading_draws[draw])))
        if loading_draws[draw, pivot] < 0.0:
            loading_draws[draw] *= -1.0
            factors[draw] *= -1.0
            sign_flips += 1
    masked_predictions = []
    errors = []
    for row in range(values.shape[0]):
        for feature in range(values.shape[1]):
            if observed[row, feature]:
                continue
            standardized_draws = factors[:, row] * loading_draws[:, feature]
            prediction_draws = standardized_draws * scales[feature] + means[feature]
            prediction_mean = float(prediction_draws.mean())
            prediction_sd = math.sqrt(
                float(prediction_draws.var(ddof=1))
                + (request["noise_standard_deviation"] * scales[feature]) ** 2
            )
            target = values[row, feature]
            errors.append((prediction_mean - target) ** 2)
            masked_predictions.append({
                "entity_id": rows[row]["entity_id"],
                "feature_name": request["feature_names"][feature],
                "posterior_mean": prediction_mean,
                "posterior_standard_deviation": prediction_sd,
                "evaluation_target": target,
                "evaluation_role": "masked_before_fit",
            })
    diagnostic_names = ["length_scale_um", "loadings", "spatial_factor"]
    rhat_values = []
    bulk_values = []
    for name in diagnostic_names:
        rhat_values.extend(np.asarray(pm.stats.rhat(posterior, var_names=[name], method="rank")[name]).ravel())
        bulk_values.extend(np.asarray(pm.stats.ess(posterior, var_names=[name], method="bulk")[name]).ravel())
    maximum_rhat = float(np.nanmax(rhat_values))
    minimum_bulk_ess = float(np.nanmin(bulk_values))
    divergences = int(np.asarray(posterior.sample_stats["diverging"], dtype=bool).sum())
    fit_state = "complete" if divergences == 0 and maximum_rhat <= 1.1 else "nonconverged"
    prior_factor = np.asarray(prior.prior["spatial_factor"], dtype=float)
    result = {
        "format": "marklab.spatial_latent_factor_model",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "matrix_id": request["matrix_id"],
        "entity_level": request["entity_level"],
        "coordinate_frame": request["coordinate_frame"],
        "kernel": {"family": "matern", "nu": 1.5, "jitter": 1e-6},
        "standardization": {"fit_entries": "observed_only", "means": means.tolist(), "scales": scales.tolist()},
        "posterior_spatial_factor": {
            "mean": factors.mean(axis=0).tolist(),
            "standard_deviation": factors.std(axis=0, ddof=1).tolist(),
        },
        "posterior_loadings": {
            "mean": loading_draws.mean(axis=0).tolist(),
            "standard_deviation": loading_draws.std(axis=0, ddof=1).tolist(),
        },
        "posterior_length_scale_um": {
            "mean": float(length_draws.mean()),
            "standard_deviation": float(length_draws.std(ddof=1)),
        },
        "alignment": {"sign": "max_loading_positive_per_draw", "sign_flips": sign_flips},
        "masked_predictions": masked_predictions,
        "masked_rmse": math.sqrt(sum(errors) / len(errors)),
        "prior_predictive": {
            "factor_standard_deviation": float(prior_factor.std(ddof=1)),
            "draws": 32,
        },
        "diagnostics": {
            "chains": 2,
            "warmup": request["warmup"],
            "draws_per_chain": request["samples"],
            "divergences": divergences,
            "maximum_rank_rhat": maximum_rhat,
            "minimum_bulk_ess": minimum_bulk_ess,
            "captured_backend_log_lines": len(captured.getvalue().splitlines()),
        },
        "fit_state": fit_state,
        "claim_status": (
            "experimental_synthetic_spatial_latent_factor"
            if fit_state == "complete"
            else "unavailable_nonconverged_spatial_latent_factor"
        ),
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"spatial latent factor worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
