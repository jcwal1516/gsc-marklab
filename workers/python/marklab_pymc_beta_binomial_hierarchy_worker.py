#!/usr/bin/env python3
"""PyMC worker for a patient beta-binomial hierarchy."""

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
from marklab_worker_support.beta_binomial_hierarchy import validate_request as validate
from marklab_worker_support.count_summaries import hierarchy_report
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

RESULT_FORMAT = "marklab.pymc_beta_binomial_hierarchy_worker_result"


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-beta-binomial-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the patient beta-binomial hierarchy by NUTS with explicit patient probabilities.

    Likelihood: ``y_i ~ Binomial(n_i, p_i)``. Patient probabilities are
    ``p_i ~ Beta(mu * phi, (1 - mu) * phi)`` around the population probability
    ``mu ~ Beta(population_alpha, population_beta)`` with concentration
    ``phi ~ HalfNormal(concentration_sd)``.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    with pm.Model():
        population = pm.Beta(
            "population_probability", config["population_alpha"], config["population_beta"]
        )
        concentration = pm.HalfNormal("concentration", config["concentration_sd"])
        patient_probability = pm.Beta(
            "patient_probability",
            population * concentration,
            (1.0 - population) * concentration,
            shape=len(config["patient_ids"]),
        )
        pm.Binomial(
            "successes", n=config["trials"], p=patient_probability, observed=config["successes"]
        )
        prior, posterior, predictive = pymc_nuts.sample_prior_posterior_predictive(
            config, seed_for, "successes"
        )
    draws = posterior["posterior"]
    population_draws = np.asarray(draws["population_probability"].values, dtype=np.float64)
    concentration_draws = np.asarray(draws["concentration"].values, dtype=np.float64)
    patient_draws = np.asarray(draws["patient_probability"].values, dtype=np.float64)
    replicated = np.asarray(predictive["posterior_predictive"]["successes"].values, dtype=np.int64)
    prior_finite = pymc_nuts.prior_is_finite(prior)
    posterior_finite = bool(
        np.isfinite(population_draws).all()
        and np.isfinite(concentration_draws).all()
        and np.isfinite(patient_draws).all()
        and np.isfinite(replicated).all()
    )
    monitored = ["population_probability", "concentration", "patient_probability"]
    convergence = pymc_nuts_diagnostics(posterior, monitored)
    report, constraints = hierarchy_report(
        config, population_draws, concentration_draws, patient_draws, replicated
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
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "Marklab PyMC beta-binomial hierarchy worker failed")
