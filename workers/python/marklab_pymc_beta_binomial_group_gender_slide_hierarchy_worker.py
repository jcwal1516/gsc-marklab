#!/usr/bin/env python3
"""PyMC worker for the slide-within-patient beta-binomial count hierarchy."""

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
from marklab_worker_support.beta_binomial_slide_hierarchy import validate_request as validate
from marklab_worker_support.count_summaries import slide_hierarchy_report
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

RESULT_FORMAT = "marklab.pymc_beta_binomial_group_gender_slide_hierarchy_worker_result"
MONITORED = [
    "intercept_log_odds",
    "group_log_odds_effect",
    "gender_log_odds_effect",
    "patient_log_odds_sd",
    "slide_concentration",
    "patient_log_odds_effect",
]


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed(
        "marklab-pymc-beta-binomial-group-gender-slide-hierarchy-v1", seed, purpose, index
    )


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the slide-within-patient beta-binomial hierarchy by NUTS.

    Likelihood: slide counts ``y_s ~ BetaBinomial(n_s, p_i * phi, (1 - p_i) * phi)`` for the
    slide's patient ``i``, with ``logit(p_i) = alpha + beta_group * g_i + beta_gender * s_i +
    u_i``. Patient effects are non-centred: ``u_i = tau * z_i`` with ``z_i ~ Normal(0, 1)`` and
    ``tau ~ HalfNormal(patient_sd_prior)``. Priors: ``alpha ~ Normal(intercept_mean,
    intercept_sd)``, ``beta_group ~ Normal(0, group_sd)``, ``beta_gender ~ Normal(0,
    gender_sd)``, and slide concentration ``phi ~ HalfNormal(concentration_sd)``.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    patient_count = len(config["patient_ids"])
    with pm.Model():
        intercept = pm.Normal(
            "intercept_log_odds", config["intercept_mean"], config["intercept_sd"]
        )
        group_effect = pm.Normal("group_log_odds_effect", 0.0, config["group_sd"])
        gender_effect = pm.Normal("gender_log_odds_effect", 0.0, config["gender_sd"])
        patient_sd = pm.HalfNormal("patient_log_odds_sd", config["patient_sd_prior"])
        patient_z = pm.Normal("patient_standardized_effect", 0.0, 1.0, shape=patient_count)
        patient_effect = pm.Deterministic("patient_log_odds_effect", patient_sd * patient_z)
        patient_probability = pm.Deterministic(
            "patient_probability",
            pm.math.sigmoid(
                intercept
                + config["patient_group_indicator"] * group_effect
                + config["patient_gender_indicator"] * gender_effect
                + patient_effect
            ),
        )
        slide_concentration = pm.HalfNormal("slide_concentration", config["concentration_sd"])
        slide_probability = patient_probability[config["patient_index"]]
        pm.BetaBinomial(
            "successes",
            alpha=slide_probability * slide_concentration,
            beta=(1.0 - slide_probability) * slide_concentration,
            n=config["trials"],
            observed=config["successes"],
        )
        prior, posterior, predictive = pymc_nuts.sample_prior_posterior_predictive(
            config, seed_for, "successes"
        )
    draws = {
        name: np.asarray(posterior["posterior"][name].values, dtype=np.float64)
        for name in [*MONITORED, "patient_probability"]
    }
    replicated = np.asarray(predictive["posterior_predictive"]["successes"].values, dtype=np.int64)
    prior_finite = pymc_nuts.prior_is_finite(prior, ("prior", "prior_predictive"))
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in [*draws.values(), replicated])
    )
    convergence = pymc_nuts_diagnostics(posterior, MONITORED)
    report, constraints = slide_hierarchy_report(config, draws, replicated)
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
    write_result(result, max_bytes=config["maximum_output_bytes"], newline=False)


if __name__ == "__main__":
    run_worker(main, "Marklab PyMC slide hierarchy worker failed")
