#!/usr/bin/env python3
"""PyMC worker for patient-unit Dirichlet-multinomial group regression."""

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
import pytensor.tensor as pt

from marklab_worker_support import pymc_nuts
from marklab_worker_support.count_summaries import (
    class_posterior_report,
    class_probabilities,
    predictive_class_report,
)
from marklab_worker_support.dirichlet_multinomial_group import validate_request as validate
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

RESULT_FORMAT = "marklab.pymc_dirichlet_multinomial_group_worker_result"


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-dirichlet-multinomial-group-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the collapsed patient Dirichlet-multinomial two-group model by NUTS.

    Likelihood: class-count vectors ``y_i ~ DirichletMultinomial(N_i, phi * pi_g(i))``. Class
    probabilities are ``pi_ref = softmax([b, 0])`` and ``pi_cmp = softmax([b + d, 0])`` with
    the last class as the zero-logit reference. Priors: baseline logits
    ``b_k ~ Normal(0, logit_sd)``, group log-ratio effects ``d_k ~ Normal(0, group_sd)``, and
    concentration ``phi ~ HalfNormal(concentration_sd)``.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    classes = len(config["class_ids"])
    with pm.Model():
        baseline = pm.Normal("baseline_logits", 0.0, config["logit_sd"], shape=classes - 1)
        effect = pm.Normal("group_log_ratio_effects", 0.0, config["group_sd"], shape=classes - 1)
        concentration = pm.HalfNormal("concentration", config["concentration_sd"])
        zero = pt.zeros((1,))
        reference_probability = pm.math.softmax(pt.concatenate([baseline, zero]))
        comparison_probability = pm.math.softmax(pt.concatenate([baseline + effect, zero]))
        group_probability = pt.stack([reference_probability, comparison_probability])
        patient_probability = group_probability[config["indicator"]]
        pm.DirichletMultinomial(
            "counts",
            n=config["totals"],
            a=patient_probability * concentration,
            observed=config["counts"],
        )
        prior, posterior, predictive = pymc_nuts.sample_prior_posterior_predictive(
            config, seed_for, "counts"
        )
    draws = posterior["posterior"]
    baseline_draws = np.asarray(draws["baseline_logits"].values, dtype=np.float64)
    effect_draws = np.asarray(draws["group_log_ratio_effects"].values, dtype=np.float64)
    concentration_draws = np.asarray(draws["concentration"].values, dtype=np.float64)
    reference_draws, comparison_draws = class_probabilities(baseline_draws, effect_draws)
    replicated = np.asarray(predictive["posterior_predictive"]["counts"].values, dtype=np.int64)
    monitored = ["baseline_logits", "group_log_ratio_effects", "concentration"]
    prior_finite = pymc_nuts.prior_is_finite(prior)
    posterior_finite = bool(
        np.isfinite(reference_draws).all()
        and np.isfinite(comparison_draws).all()
        and np.isfinite(concentration_draws).all()
        and np.isfinite(replicated).all()
    )
    convergence = pymc_nuts_diagnostics(posterior, monitored)
    posterior_report, constraints = class_posterior_report(
        config["class_ids"],
        reference_draws,
        comparison_draws,
        concentration_draws,
        config["chains"],
    )
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        **fit_summary(convergence, config, prior_finite, posterior_finite, constraints),
        "posterior": posterior_report,
        "posterior_predictive": {"classes": predictive_class_report(config, replicated)},
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(2 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=2 * MIB)


if __name__ == "__main__":
    run_worker(main, "Marklab PyMC Dirichlet-multinomial group worker failed")
