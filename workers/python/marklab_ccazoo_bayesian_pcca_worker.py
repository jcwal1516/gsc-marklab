#!/usr/bin/env python3
"""Bayesian probabilistic CCA of two paired Gaussian views with CCA-Zoo and NumPyro NUTS."""

import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import cca_zoo
import jax
import numpy as np
import numpyro
import scipy
from cca_zoo.probabilistic import ProbabilisticCCA
from numpyro.diagnostics import effective_sample_size
from numpyro.infer import MCMC, NUTS

from marklab_worker_support.diagnostics import divergence_count
from marklab_worker_support.factor_models import standardize_paired_rows
from marklab_worker_support.protocol import (
    CONTRACT_ERRORS,
    MIB,
    ContractError,
    derive_seed,
    exact,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    write_result,
)


def jax_seed(seed: int) -> int:
    """Map the full request ``u64`` range to a seed accepted by JAX.

    Preserve conventional uint32 seeds exactly. JAX otherwise reduces Python integers modulo
    ``2**32`` (and rejects the upper half of ``u64``), so deterministically derive a uint32 seed
    for every larger request seed instead of allowing periodic key collisions.
    """
    if seed <= 2**32 - 1:
        return seed
    return derive_seed("marklab-ccazoo-bayesian-pcca-v1", seed, "jax-seed")


def posterior_scores(observations, loadings, variance):
    """Posterior mean of the latent scores given loadings and diagonal noise variances."""
    precision_loadings = loadings / variance[:, None]
    middle = np.eye(loadings.shape[1]) + loadings.T @ precision_loadings
    return observations @ precision_loadings @ np.linalg.inv(middle)


def sample_bayesian_pcca(training_views, request):
    """Sample CCA-Zoo's one-factor probabilistic CCA by NUTS (one chain).

    Model for view ``i`` (standardized, not re-centred): ``z_n ~ Normal(0, 1)``,
    ``W_i ~ Normal(0, 1)`` elementwise, ``log_psi_i ~ Normal(0, 1)`` per feature, and
    ``x_in ~ Normal(W_i z_n, exp(log_psi_i))`` with the noise SD ``exp(log_psi_i)``.
    """
    seed = jax_seed(request["seed"])
    model = ProbabilisticCCA(
        latent_dimensions=1,
        center=False,
        num_warmup=request["warmup"],
        num_samples=request["samples"],
        random_state=seed,
    )
    validated = model._setup_fit(training_views)
    kernel = NUTS(model._model, target_accept_prob=request["target_accept"])
    mcmc = MCMC(
        kernel,
        num_warmup=request["warmup"],
        num_samples=request["samples"],
        progress_bar=False,
    )
    mcmc.run(jax.random.PRNGKey(seed), validated)
    samples = {key: np.asarray(value) for key, value in mcmc.get_samples().items()}
    extra = {key: np.asarray(value) for key, value in mcmc.get_extra_fields().items()}
    return samples, extra


def main() -> None:
    require_versions(
        cca_zoo=cca_zoo.__version__,
        numpyro=numpyro.__version__,
        jax=jax.__version__,
        numpy=np.__version__,
        scipy=scipy.__version__,
    )
    request_bytes = read_request(16 * MIB)
    request = json.loads(request_bytes)
    exact(request["format"], "marklab.ccazoo_bayesian_pcca_request", "request.format")
    exact(request["version"], 1, "request.version")
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
    paired = standardize_paired_rows(rows)
    train = paired.train
    test = paired.test
    training_views = [paired.standardized_x[train], paired.standardized_y[train]]

    samples, extra = sample_bayesian_pcca(training_views, request)
    divergences = divergence_count(extra.get("diverging", np.zeros(request["samples"], dtype=bool)))

    # Resolve the sign non-identifiability draw by draw: the largest-magnitude first-view
    # loading is made positive, flipping both loadings and the latent scores together.
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
    scores_x = posterior_scores(paired.standardized_x, mean_w0, variance_x)
    scores_y = posterior_scores(paired.standardized_y, mean_w1, variance_y)
    correlation = float(np.corrcoef(scores_x[train, 0], scores_y[train, 0])[0, 1])
    predicted_y = (scores_x @ mean_w1.T) * paired.scales_y + paired.means_y
    heldout_rmse = float(np.sqrt(np.mean((predicted_y[test] - paired.y[test]) ** 2)))

    ess_values = []
    for values in [aligned_w0, aligned_w1, samples["log_psi_0"], samples["log_psi_1"]]:
        ess_values.extend(np.asarray(effective_sample_size(values[None, ...])).ravel().tolist())
    finite_ess = [float(value) for value in ess_values if math.isfinite(float(value))]
    minimum_ess = min(finite_ess) if finite_ess else 0.0
    converged = divergences == 0
    result = {
        "format": "marklab.bayesian_pcca",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "design": {**design, "validation_status": "passed", "paired_row_count": len(rows)},
        "standardization": paired.standardization_record(),
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
        "fit_state": "complete" if converged else "nonconverged",
        "claim_status": (
            "experimental_synthetic_bayesian_pcca"
            if converged
            else "unsupported_for_claim_nonconverged"
        ),
    }
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "Bayesian pCCA worker contract error", errors=CONTRACT_ERRORS)
