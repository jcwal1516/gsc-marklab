#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import cca_zoo
import jax
import numpy as np
import numpyro
import scipy
from cca_zoo.probabilistic import ProbabilisticCCA
from numpyro.diagnostics import effective_sample_size
from numpyro.infer import MCMC, NUTS


class ContractError(Exception):
    pass


def posterior_scores(observations, loadings, variance):
    precision_loadings = loadings / variance[:, None]
    middle = np.eye(loadings.shape[1]) + loadings.T @ precision_loadings
    return observations @ precision_loadings @ np.linalg.inv(middle)


def main():
    if (
        cca_zoo.__version__ != "3.0.0"
        or numpyro.__version__ != "0.21.0"
        or jax.__version__ != "0.11.1"
        or np.__version__ != "2.4.6"
        or scipy.__version__ != "1.18.1"
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("CCA-Zoo/NumPyro/JAX/NumPy/SciPy/Python version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.ccazoo_bayesian_pcca_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    design = request["design"]
    if (
        design["entity_level"] != "patient"
        or design["modality_x"]["measurement_status"] != "measured"
        or design["modality_y"]["measurement_status"] != "measured"
        or design["modality_x"]["likelihood"] != "gaussian"
        or design["modality_y"]["likelihood"] != "gaussian"
        or design["missingness_assumption"] != "complete_paired_rows"
        or request["priors"] != "cca_zoo_standard_normal_loadings_log_noise"
        or request["latent_dimensions"] != 1
    ):
        raise ContractError("Bayesian pCCA design or prior contract is invalid")
    rows = request["rows"]
    x = np.asarray([row["x"] for row in rows], dtype=float)
    y = np.asarray([row["y"] for row in rows], dtype=float)
    train = np.asarray([row["split"] == "train" for row in rows], dtype=bool)
    test = np.asarray([row["split"] == "test" for row in rows], dtype=bool)
    if train.sum() < 8 or test.sum() < 1 or not np.isfinite(x).all() or not np.isfinite(y).all():
        raise ContractError("Bayesian pCCA requires finite training and held-out rows")
    means_x = x[train].mean(axis=0)
    means_y = y[train].mean(axis=0)
    scales_x = x[train].std(axis=0, ddof=0)
    scales_y = y[train].std(axis=0, ddof=0)
    if np.any(scales_x <= 0.0) or np.any(scales_y <= 0.0):
        raise ContractError("training features must vary")
    standardized_x = (x - means_x) / scales_x
    standardized_y = (y - means_y) / scales_y
    training_views = [standardized_x[train], standardized_y[train]]

    model = ProbabilisticCCA(
        latent_dimensions=1,
        center=False,
        num_warmup=request["warmup"],
        num_samples=request["samples"],
        random_state=request["seed"],
    )
    validated = model._setup_fit(training_views)
    kernel = NUTS(model._model, target_accept_prob=request["target_accept"])
    mcmc = MCMC(
        kernel,
        num_warmup=request["warmup"],
        num_samples=request["samples"],
        progress_bar=False,
    )
    mcmc.run(jax.random.PRNGKey(request["seed"]), validated)
    samples = {key: np.asarray(value) for key, value in mcmc.get_samples().items()}
    extra = {key: np.asarray(value) for key, value in mcmc.get_extra_fields().items()}
    divergences = int(extra.get("diverging", np.zeros(request["samples"], dtype=bool)).sum())

    aligned_w0 = samples["W_0"].copy()
    aligned_w1 = samples["W_1"].copy()
    aligned_z = samples["z"].copy()
    sign_flips = 0
    for draw in range(request["samples"]):
        pivot = int(np.argmax(np.abs(aligned_w0[draw, :, 0])))
        if aligned_w0[draw, pivot, 0] < 0.0:
            aligned_w0[draw] *= -1.0
            aligned_w1[draw] *= -1.0
            aligned_z[draw] *= -1.0
            sign_flips += 1
    mean_w0 = aligned_w0.mean(axis=0)
    mean_w1 = aligned_w1.mean(axis=0)
    variance_x = np.mean(np.exp(2.0 * samples["log_psi_0"]), axis=0)
    variance_y = np.mean(np.exp(2.0 * samples["log_psi_1"]), axis=0)
    scores_x = posterior_scores(standardized_x, mean_w0, variance_x)
    scores_y = posterior_scores(standardized_y, mean_w1, variance_y)
    correlation = float(np.corrcoef(scores_x[train, 0], scores_y[train, 0])[0, 1])
    predicted_y = (scores_x @ mean_w1.T) * scales_y + means_y
    heldout_rmse = float(np.sqrt(np.mean((predicted_y[test] - y[test]) ** 2)))
    ess_values = []
    for values in [aligned_w0, aligned_w1, samples["log_psi_0"], samples["log_psi_1"]]:
        ess_values.extend(np.asarray(effective_sample_size(values[None, ...])).ravel().tolist())
    finite_ess = [float(value) for value in ess_values if math.isfinite(float(value))]
    minimum_ess = min(finite_ess) if finite_ess else 0.0
    result = {
        "format": "marklab.bayesian_pcca",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "design": {**design, "validation_status": "passed", "paired_row_count": len(rows)},
        "standardization": {
            "fit_split": "train_only",
            "fit_row_count": int(train.sum()),
            "mean_x": means_x.tolist(),
            "scale_x": scales_x.tolist(),
            "mean_y": means_y.tolist(),
            "scale_y": scales_y.tolist(),
        },
        "posterior": {
            "loadings_x_mean": mean_w0.tolist(),
            "loadings_x_sd": aligned_w0.std(axis=0, ddof=1).tolist(),
            "loadings_y_mean": mean_w1.tolist(),
            "loadings_y_sd": aligned_w1.std(axis=0, ddof=1).tolist(),
            "noise_variance_x_mean": variance_x.tolist(),
            "noise_variance_y_mean": variance_y.tolist(),
            "latent_score_mean_train": aligned_z.mean(axis=0).tolist(),
        },
        "alignment": {
            "method": "first_view_max_loading_positive",
            "aligned_draw_count": request["samples"],
            "sign_flips": sign_flips,
            "rotation_limitation": "version_one_supports_one_latent_dimension_only",
        },
        "posterior_mean_canonical_correlation": correlation,
        "heldout_row_count": int(test.sum()),
        "heldout_cross_view_rmse_y_from_x": heldout_rmse,
        "diagnostics": {
            "chains": 1,
            "warmup": request["warmup"],
            "samples": request["samples"],
            "divergences": divergences,
            "minimum_effective_sample_size": minimum_ess,
            "rhat_status": "unavailable_single_chain",
            "target_accept": request["target_accept"],
        },
        "fit_state": "complete" if divergences == 0 else "nonconverged",
        "claim_status": "experimental_synthetic_bayesian_pcca" if divergences == 0 else "unsupported_for_claim_nonconverged",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"Bayesian pCCA worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
