#!/usr/bin/env python3
"""PyMC worker for patient-unit beta-binomial group regression."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

# Configure PyTensor before PyMC is imported.
from marklab_worker_support import pytensor_config  # noqa: F401

# isort: split
import numpy as np
import pymc as pm

from marklab_worker_support import pymc_nuts
from marklab_worker_support.beta_binomial_group import validate_request as validate
from marklab_worker_support.count_summaries import group_report
from marklab_worker_support.nuts_contract import fit_summary, pymc_nuts_diagnostics
from marklab_worker_support.protocol import (
    MIB,
    backend_identity,
    derive_seed,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)

RESULT_FORMAT = "marklab.pymc_beta_binomial_group_regression_worker_result"


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-beta-binomial-group-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the collapsed patient beta-binomial two-group regression by NUTS.

    Likelihood: ``y_i ~ BetaBinomial(n_i, p_i * phi, (1 - p_i) * phi)`` with
    ``logit(p_i) = alpha + beta * g_i`` for comparison-group indicator ``g_i``. Priors:
    ``alpha ~ Normal(intercept_mean, intercept_sd)``, ``beta ~ Normal(0, group_sd)``, and
    concentration ``phi ~ HalfNormal(concentration_sd)``. Patient probabilities are integrated
    out and drawn afterwards from their exact conditional beta posteriors.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    with pm.Model():
        intercept = pm.Normal(
            "intercept_log_odds", config["intercept_mean"], config["intercept_sd"]
        )
        effect = pm.Normal("group_log_odds_effect", 0.0, config["group_sd"])
        concentration = pm.HalfNormal("concentration", config["concentration_sd"])
        group_probability = pm.math.sigmoid(intercept + config["indicator"] * effect)
        pm.BetaBinomial(
            "successes",
            alpha=group_probability * concentration,
            beta=(1.0 - group_probability) * concentration,
            n=config["trials"],
            observed=config["successes"],
        )
        prior, posterior, predictive = pymc_nuts.sample_prior_posterior_predictive(
            config, seed_for, "successes"
        )
    draws = posterior["posterior"]
    intercept_draws = np.asarray(draws["intercept_log_odds"].values, dtype=np.float64)
    effect_draws = np.asarray(draws["group_log_odds_effect"].values, dtype=np.float64)
    concentration_draws = np.asarray(draws["concentration"].values, dtype=np.float64)
    replicated = np.asarray(predictive["posterior_predictive"]["successes"].values, dtype=np.int64)
    monitored = ["intercept_log_odds", "group_log_odds_effect", "concentration"]
    prior_finite = pymc_nuts.prior_is_finite(prior)
    posterior_finite = bool(
        np.isfinite(intercept_draws).all()
        and np.isfinite(effect_draws).all()
        and np.isfinite(concentration_draws).all()
        and np.isfinite(replicated).all()
    )
    convergence = pymc_nuts_diagnostics(posterior, monitored)
    report, constraints = group_report(
        config,
        intercept_draws,
        effect_draws,
        concentration_draws,
        replicated,
        np.random.default_rng(seed_for(config["seed"], "conditional")),
    )
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        **fit_summary(convergence, config, prior_finite, posterior_finite, constraints),
        **report,
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(2 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=2 * MIB)


if __name__ == "__main__":
    run_worker(main, "Marklab PyMC beta-binomial group worker failed")
