#!/usr/bin/env python3
"""PyMC worker for Marklab's Gaussian patient-hierarchy contract."""

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

from marklab_worker_support.normal_models import (
    hierarchy_summaries,
    patient_index,
    validate_hierarchy_request,
)
from marklab_worker_support.nuts_contract import (
    convergence_passes,
    pymc_nuts_diagnostics,
)
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
from marklab_worker_support.pymc_nuts import sample_nuts

RESULT_FORMAT = "marklab.pymc_hierarchical_worker_result"
CONTRACT_VERSION = 1


def derived_seed(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-hierarchical-v1", seed, purpose, index)


def fit_hierarchy(
    config: dict[str, Any], request_sha256: str, lock_digest: str, worker_digest: str
) -> dict[str, Any]:
    """Fit the Gaussian patient varying-intercept hierarchy by NUTS.

    Model: ``global_mean ~ Normal(global_prior_mean, global_prior_sd)``,
    ``between_patient_sd ~ HalfNormal(between_prior_sd)``, non-centred patient means
    ``patient_mean_j = global_mean + between_patient_sd * z_j`` with ``z_j ~ Normal(0, 1)``,
    and observations ``y_ij ~ Normal(patient_mean_j, known_sigma)``.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    observation_patient = patient_index(config["patient_values"], np.int64)
    observations = np.concatenate(config["patient_values"])
    chain_seeds = [
        derived_seed(config["seed"], "chain", index) for index in range(config["chains"])
    ]
    with pm.Model(coords={"patient": config["patient_ids"]}):
        global_mean = pm.Normal(
            "global_mean", mu=config["global_prior_mean"], sigma=config["global_prior_sd"]
        )
        between_patient_sd = pm.HalfNormal("between_patient_sd", sigma=config["between_prior_sd"])
        patient_z = pm.Normal("patient_z", mu=0.0, sigma=1.0, dims="patient")
        patient_mean = pm.Deterministic(
            "patient_mean", global_mean + between_patient_sd * patient_z, dims="patient"
        )
        pm.Normal(
            "y",
            mu=patient_mean[observation_patient],
            sigma=config["known_sigma"],
            observed=observations,
        )
        prior = pm.sample_prior_predictive(
            draws=config["prior_draws"],
            random_seed=derived_seed(config["seed"], "prior"),
            return_inferencedata=True,
        )
        fit = sample_nuts(config, chain_seeds)
        predictive = pm.sample_posterior_predictive(
            fit,
            var_names=["y"],
            random_seed=derived_seed(config["seed"], "posterior_predictive"),
            progressbar=False,
            return_inferencedata=True,
        )

    monitored = ["global_mean", "between_patient_sd", "patient_mean"]
    prior_finite = bool(
        all(np.isfinite(prior["prior"][name].values).all() for name in monitored)
        and np.isfinite(prior["prior_predictive"]["y"].values).all()
    )
    global_draws = np.asarray(fit["posterior"]["global_mean"].values, dtype=np.float64)
    between_draws = np.asarray(fit["posterior"]["between_patient_sd"].values, dtype=np.float64)
    patient_draws = np.asarray(fit["posterior"]["patient_mean"].values, dtype=np.float64)
    predictive_draws = np.asarray(predictive["posterior_predictive"]["y"].values, dtype=np.float64)
    posterior_finite = bool(
        np.isfinite(global_draws).all()
        and np.isfinite(between_draws).all()
        and np.isfinite(patient_draws).all()
        and np.isfinite(predictive_draws).all()
    )
    diagnostics = pymc_nuts_diagnostics(fit, monitored)
    complete = prior_finite and posterior_finite and convergence_passes(config, diagnostics)
    summaries = hierarchy_summaries(
        config,
        observations,
        observation_patient,
        global_draws,
        between_draws,
        patient_draws,
        predictive_draws,
    )
    return {
        "format": RESULT_FORMAT,
        "version": CONTRACT_VERSION,
        "backend": backend_identity("pymc", pm.__version__, lock_digest, worker_digest),
        "request_sha256": request_sha256,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        **summaries,
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            **diagnostics,
            "constraints_valid": bool(np.all(between_draws >= 0.0)),
            "identifiability_checks_passed": True,
        },
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw_request = read_request(16 * MIB)
    config = validate_hierarchy_request(json.loads(raw_request), lock_digest, worker_digest)
    result = fit_hierarchy(config, sha256_hex(raw_request), lock_digest, worker_digest)
    write_result(result, max_bytes=config["maximum_output_bytes"])


if __name__ == "__main__":
    run_worker(main, "marklab PyMC worker failed")
