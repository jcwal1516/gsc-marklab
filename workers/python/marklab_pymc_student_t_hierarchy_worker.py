#!/usr/bin/env python3
"""PyMC worker for a robust Student-t patient hierarchy."""

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
from marklab_worker_support.nuts_contract import fit_summary, pymc_nuts_diagnostics
from marklab_worker_support.posterior import summary
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
from marklab_worker_support.student_t_hierarchy import (
    hierarchy_summaries,
    student_t_scale_from_sd,
    validate_request,
)

RESULT_FORMAT = "marklab.pymc_student_t_hierarchy_worker_result"
MONITORED = [
    "global_mean",
    "between_patient_sd",
    "patient_mean",
    "observation_sd",
    "degrees_of_freedom",
]


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    """The request contract, shared with the NumPyro agreement and SBC workers."""
    return validate_request(request, lock_digest, worker_digest)


def prior_predictive_is_finite(prior: Any) -> bool:
    """Whether the prior parameter draws and the generated observations are all finite."""
    return pymc_nuts.prior_is_finite(prior, ("prior", "prior_predictive"))


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-student-t-hierarchy-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the Student-t patient hierarchy by NUTS.

    Likelihood: ``y_ij ~ StudentT(nu, mu_i, s)`` with
    ``s = sigma_obs * sqrt((nu - 2) / nu)`` so the finite observation SD is ``sigma_obs``.
    Patient means are non-centered, ``mu_i = mu + tau * z_i``
    with ``z_i ~ Normal(0, 1)``. Priors: ``mu ~ Normal(global_mean, global_sd)``,
    ``tau ~ HalfNormal(between_sd)``, ``sigma_obs ~ HalfNormal(observation_sd)``, and
    ``nu = 2 + e`` with ``e ~ Exponential(rate=df_rate)``.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    patient_index = np.concatenate(
        [
            np.full(len(values), index, dtype=np.int64)
            for index, values in enumerate(config["patient_values"])
        ]
    )
    observations = np.concatenate(config["patient_values"])
    with pm.Model():
        global_mean = pm.Normal("global_mean", config["global_mean"], config["global_sd"])
        between_sd = pm.HalfNormal("between_patient_sd", config["between_sd"])
        patient_z = pm.Normal("patient_z", 0.0, 1.0, shape=len(config["patient_ids"]))
        patient_mean = pm.Deterministic("patient_mean", global_mean + between_sd * patient_z)
        observation_sd = pm.HalfNormal("observation_sd", config["observation_sd"])
        df_excess = pm.Exponential("degrees_of_freedom_excess", config["df_rate"])
        degrees_of_freedom = pm.Deterministic("degrees_of_freedom", 2.0 + df_excess)
        observation_scale = student_t_scale_from_sd(observation_sd, degrees_of_freedom)
        pm.StudentT(
            "observation",
            nu=degrees_of_freedom,
            mu=patient_mean[patient_index],
            sigma=observation_scale,
            observed=observations,
        )
        prior, posterior, predictive = pymc_nuts.sample_prior_posterior_predictive(
            config, seed_for, "observation"
        )
    draws = {
        name: np.asarray(posterior["posterior"][name].values, dtype=np.float64)
        for name in MONITORED
    }
    replicated = np.asarray(
        predictive["posterior_predictive"]["observation"].values, dtype=np.float64
    ).reshape(-1, observations.size)
    prior_finite = prior_predictive_is_finite(prior)
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in draws.values()) and np.isfinite(replicated).all()
    )
    constraints = bool(
        np.all(draws["between_patient_sd"] > 0.0)
        and np.all(draws["observation_sd"] > 0.0)
        and np.all(draws["degrees_of_freedom"] > 2.0)
    )
    convergence = pymc_nuts_diagnostics(posterior, MONITORED)
    patient_draws = draws["patient_mean"].reshape(-1, len(config["patient_ids"]))
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        **fit_summary(convergence, config, prior_finite, posterior_finite, constraints),
        "posterior": {
            name: summary(draws[name], chains=config["chains"])
            for name in [
                "global_mean",
                "between_patient_sd",
                "observation_sd",
                "degrees_of_freedom",
            ]
        },
        **hierarchy_summaries(
            config,
            observations,
            patient_index,
            draws["global_mean"],
            patient_draws,
            replicated,
        ),
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(2 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "Marklab PyMC Student-t hierarchy worker failed")
