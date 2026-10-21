#!/usr/bin/env python3
"""PyMC worker for a one-factor spatial latent model with masked-entry prediction.

The spatial factor has either a dense Matern-3/2 GP prior over the entity coordinates (the
default) or, with ``latent_field.kind == "spde_alpha_two"``, the Lindgren, Rue and Lindstrom
(2011) SPDE prior with smoothness 1 on a regular rectangular mesh, projected to the entities by
barycentric interpolation.
"""

import contextlib
import io
import json
import math
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

# Configure PyTensor before PyMC is imported.
from marklab_worker_support import pytensor_config  # noqa: F401

# isort: split
import numpy as np
import pymc as pm
import pytensor
import pytensor.tensor as pt
from pytensor.tensor.slinalg import solve_triangular

from marklab_worker_support import fem
from marklab_worker_support.factor_models import observed_feature_standardization
from marklab_worker_support.protocol import (
    CONTRACT_ERRORS,
    MIB,
    ContractError,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    write_result,
)

JITTER = 1e-6
PRIOR_DRAWS = 32
CHAINS = 2


def spde_mesh(request: dict[str, Any], coordinates: np.ndarray) -> dict[str, Any] | None:
    """The declared SPDE mesh and projection, or ``None`` for the dense Matern-3/2 prior.

    The mesh is a regular ``mesh_resolution``-square Delaunay mesh over the entities' bounding
    box widened by ``mesh_extension_um`` on every side, which moves the natural (Neumann)
    boundary away from the entities.
    """
    field = request.get("latent_field")
    if field is None:
        return None
    if field.get("kind") != "spde_alpha_two" or request["matern_nu"] != 1.0:
        raise ContractError("SPDE latent field requires kind spde_alpha_two and matern_nu 1")
    extension = float(field["mesh_extension_um"])
    bounds = [
        float(coordinates[:, 0].min()) - extension,
        float(coordinates[:, 0].max()) + extension,
        float(coordinates[:, 1].min()) - extension,
        float(coordinates[:, 1].max()) + extension,
    ]
    mesh = fem.build_mesh(bounds, int(field["mesh_resolution"]), 1.0, 1.0)
    return {**mesh, "bounds": bounds, "projection": fem.projection(mesh, coordinates)}


def spde_factor(mesh: dict[str, Any], range_um: Any) -> Any:
    """Projected SPDE factor with unit marginal variance and practical range ``range_um``.

    ``kappa = sqrt(8) / range``, ``tau^2 = 1 / (4 pi kappa^2)`` and
    ``Q = tau^2 (kappa^4 C + 2 kappa^2 G + G C~^-1 G)`` with the lumped mass ``C~``. Mesh weights
    are ``w = L^-T z`` for ``Q = L L^T`` and ``z ~ Normal(0, I)``.
    """
    mass = mesh["mass"]
    stiffness = mesh["stiffness"]
    second_order = stiffness @ np.diag(1.0 / mass.sum(axis=1)) @ stiffness
    kappa = np.sqrt(8.0) / range_um
    precision = (kappa**2 * mass + 2.0 * stiffness + second_order / kappa**2) / (4.0 * np.pi)
    vertices = len(mass)
    cholesky = pt.linalg.cholesky(precision + JITTER * np.eye(vertices))
    raw = pm.Normal("field_raw", 0.0, 1.0, shape=vertices)
    weights = solve_triangular(cholesky.T, raw, lower=False)
    return pm.math.dot(mesh["projection"], weights)


def sample_model(
    request: dict[str, Any],
    coordinates: np.ndarray,
    standardized: np.ndarray,
    observed: np.ndarray,
) -> tuple[Any, Any, int]:
    """Sample the prior predictive and the NUTS posterior of the spatial factor model.

    Likelihood: each observed standardized entry ``y_rk ~ Normal(f_r * w_k, noise_sd)``. With
    the dense prior the spatial factor is ``f = L z`` with ``z ~ Normal(0, 1)`` and ``L`` the
    Cholesky factor of a Matern-3/2 covariance over the entity coordinates (plus jitter) and
    ``length_scale_um ~ HalfNormal(length_scale_prior_um)``. With the SPDE prior it is
    ``spde_factor`` and ``range_um ~ HalfNormal(length_scale_prior_um)``. Loadings are
    ``w_k ~ Normal(0, 1)``. Returns the prior, the posterior, and the number of log lines PyMC
    wrote while sampling.
    """
    mesh = spde_mesh(request, coordinates)
    rows = len(coordinates)
    observed_rows, observed_features = np.nonzero(observed)
    captured = io.StringIO()
    with contextlib.redirect_stdout(captured), contextlib.redirect_stderr(captured):
        with pm.Model():
            if mesh is None:
                length_scale_um = pm.HalfNormal(
                    "length_scale_um", sigma=request["length_scale_prior_um"]
                )
                covariance = pm.gp.cov.Matern32(input_dim=2, ls=length_scale_um)(coordinates)
                cholesky = pm.math.cholesky(covariance + JITTER * np.eye(rows))
                factor_raw = pm.Normal("factor_raw", 0.0, 1.0, shape=rows)
                factor = pm.math.dot(cholesky, factor_raw)
            else:
                range_um = pm.HalfNormal("range_um", sigma=request["length_scale_prior_um"])
                factor = spde_factor(mesh, range_um)
            spatial_factor = pm.Deterministic("spatial_factor", factor)
            loadings = pm.Normal("loadings", 0.0, 1.0, shape=standardized.shape[1])
            linear_predictor = spatial_factor[:, None] * loadings[None, :]
            pm.Normal(
                "observed_values",
                mu=linear_predictor[observed_rows, observed_features],
                sigma=request["noise_standard_deviation"],
                observed=standardized[observed],
            )
            prior = pm.sample_prior_predictive(draws=PRIOR_DRAWS, random_seed=request["seed"])
            posterior = pm.sample(
                draws=request["samples"],
                tune=request["warmup"],
                chains=CHAINS,
                cores=1,
                target_accept=request["target_accept"],
                random_seed=[request["seed"], request["seed"] + 1],
                progressbar=False,
                compute_convergence_checks=False,
                return_inferencedata=True,
                nuts_sampler="pymc",
            )
    return prior, posterior, len(captured.getvalue().splitlines()), mesh


def align_signs(factors: np.ndarray, loadings: np.ndarray) -> int:
    """Flip each draw, in place, so its largest-magnitude loading is positive; count flips."""
    sign_flips = 0
    for draw in range(len(factors)):
        pivot = int(np.argmax(np.abs(loadings[draw])))
        if loadings[draw, pivot] < 0.0:
            loadings[draw] *= -1.0
            factors[draw] *= -1.0
            sign_flips += 1
    return sign_flips


def masked_predictions(
    request: dict[str, Any],
    values: np.ndarray,
    observed: np.ndarray,
    means: np.ndarray,
    scales: np.ndarray,
    factors: np.ndarray,
    loadings: np.ndarray,
) -> tuple[list[dict[str, Any]], list[float]]:
    """Predict every masked entry on the original scale; return the rows and squared errors."""
    predictions = []
    errors = []
    noise_sd = request["noise_standard_deviation"]
    for row in range(values.shape[0]):
        for feature in range(values.shape[1]):
            if observed[row, feature]:
                continue
            standardized_draws = factors[:, row] * loadings[:, feature]
            prediction_draws = standardized_draws * scales[feature] + means[feature]
            prediction_mean = float(prediction_draws.mean())
            prediction_sd = math.sqrt(
                float(prediction_draws.var(ddof=1)) + (noise_sd * scales[feature]) ** 2
            )
            target = values[row, feature]
            errors.append((prediction_mean - target) ** 2)
            predictions.append(
                {
                    "entity_id": request["rows"][row]["entity_id"],
                    "feature_name": request["feature_names"][feature],
                    "posterior_mean": prediction_mean,
                    "posterior_standard_deviation": prediction_sd,
                    "evaluation_target": target,
                    "evaluation_role": "masked_before_fit",
                }
            )
    return predictions, errors


def convergence(posterior: Any, scale_name: str) -> tuple[float, float]:
    """Maximum rank R-hat and minimum bulk ESS over every monitored scalar, ignoring NaN."""
    rhat_values = []
    bulk_values = []
    for name in [scale_name, "loadings", "spatial_factor"]:
        rhat = pm.stats.rhat(posterior, var_names=[name], method="rank")[name]
        bulk = pm.stats.ess(posterior, var_names=[name], method="bulk")[name]
        rhat_values.extend(np.asarray(rhat).ravel())
        bulk_values.extend(np.asarray(bulk).ravel())
    return float(np.nanmax(rhat_values)), float(np.nanmin(bulk_values))


def main() -> None:
    require_versions(pymc=pm.__version__, pytensor=pytensor.__version__, numpy=np.__version__)
    request_bytes = read_request(16 * MIB)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.pymc_spatial_latent_factor_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    rows = request["rows"]
    values = np.asarray([row["values"] for row in rows], dtype=float)
    observed = np.asarray([row["observed"] for row in rows], dtype=bool)
    coordinates = np.asarray([row["coordinates_um"] for row in rows], dtype=float)
    means, scales = observed_feature_standardization(values, observed, "spatial latent")
    standardized = (values - means) / scales
    prior, posterior, log_lines, mesh = sample_model(request, coordinates, standardized, observed)
    scale_name = "length_scale_um" if mesh is None else "range_um"
    factors = np.asarray(posterior.posterior["spatial_factor"], dtype=float).reshape(-1, len(rows))
    loading_draws = np.asarray(posterior.posterior["loadings"], dtype=float).reshape(
        -1, values.shape[1]
    )
    length_draws = np.asarray(posterior.posterior[scale_name], dtype=float).reshape(-1)
    sign_flips = align_signs(factors, loading_draws)
    predictions, errors = masked_predictions(
        request, values, observed, means, scales, factors, loading_draws
    )
    maximum_rhat, minimum_bulk_ess = convergence(posterior, scale_name)
    divergences = int(np.asarray(posterior.sample_stats["diverging"], dtype=bool).sum())
    fit_state = "complete" if divergences == 0 and maximum_rhat <= 1.1 else "nonconverged"
    prior_factor = np.asarray(prior.prior["spatial_factor"], dtype=float)
    result = {
        "format": "marklab.spatial_latent_factor_model",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "matrix_id": request["matrix_id"],
        "entity_level": request["entity_level"],
        "coordinate_frame": request["coordinate_frame"],
        "kernel": (
            {"family": "matern", "nu": 1.5, "jitter": JITTER}
            if mesh is None
            else {
                "family": "spde_matern",
                "alpha": 2,
                "nu": 1.0,
                "jitter": JITTER,
                "mesh_bounds_um": mesh["bounds"],
                "mesh_vertices": len(mesh["vertices"]),
                "mesh_triangles": len(mesh["triangles"]),
                "projection": "barycentric",
                "boundary_condition": "natural_neumann_on_extended_rectangle",
                "variance_normalization": "unit_marginal_variance_on_the_plane",
            }
        ),
        "standardization": {
            "fit_entries": "observed_only",
            "means": means.tolist(),
            "scales": scales.tolist(),
        },
        "posterior_spatial_factor": {
            "mean": factors.mean(axis=0).tolist(),
            "standard_deviation": factors.std(axis=0, ddof=1).tolist(),
        },
        "posterior_loadings": {
            "mean": loading_draws.mean(axis=0).tolist(),
            "standard_deviation": loading_draws.std(axis=0, ddof=1).tolist(),
        },
        f"posterior_{scale_name}": {
            "mean": float(length_draws.mean()),
            "standard_deviation": float(length_draws.std(ddof=1)),
        },
        "alignment": {"sign": "max_loading_positive_per_draw", "sign_flips": sign_flips},
        "masked_predictions": predictions,
        "masked_rmse": math.sqrt(sum(errors) / len(errors)),
        "prior_predictive": {
            "factor_standard_deviation": float(prior_factor.std(ddof=1)),
            "draws": PRIOR_DRAWS,
        },
        "diagnostics": {
            "chains": CHAINS,
            "warmup": request["warmup"],
            "draws_per_chain": request["samples"],
            "divergences": divergences,
            "maximum_rank_rhat": maximum_rhat,
            "minimum_bulk_ess": minimum_bulk_ess,
            "captured_backend_log_lines": log_lines,
        },
        "fit_state": fit_state,
        "claim_status": (
            "experimental_synthetic_spatial_latent_factor"
            if fit_state == "complete"
            else "unavailable_nonconverged_spatial_latent_factor"
        ),
    }
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "spatial latent factor worker contract error", errors=CONTRACT_ERRORS)
