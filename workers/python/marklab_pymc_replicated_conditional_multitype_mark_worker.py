#!/usr/bin/env python3
"""Pinned PyMC patient-hierarchical conditional model for hard multitype marks."""

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

from marklab_worker_support.conditional_mark import (
    HIERARCHY_PARAMETERS,
    policy_met,
    potential_design,
    replicated_posterior,
    validate_replicated_request,
)
from marklab_worker_support.diagnostics import (
    mcse_extremes,
    pymc_sampler_diagnostics,
    rank_convergence,
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

RESULT_FORMAT = "marklab.pymc_replicated_conditional_multitype_mark_result"


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed(
        "marklab-pymc-replicated-conditional-multitype-mark-v1", seed, purpose, index
    )


def flatten(trace: Any, name: str) -> np.ndarray:
    value = np.asarray(trace["posterior"][name].values, dtype=np.float64)
    return value.reshape((-1, *value.shape[2:]))


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the patient/pattern hierarchical conditional multitype pseudolikelihood by NUTS.

    Likelihood: each point's type is ``Categorical(softmax(intercept_p + counts @ potential_p.T))``
    with pattern-specific free intercepts and potentials (type 0 reference, symmetric potential).
    Each free vector is ``baseline + group * group_shift + patient_sd * patient_z[patient] +
    pattern_sd * pattern_z`` (non-centred). Priors: baselines ``~ Normal(0, intercept_sd)`` and
    ``Normal(0, interaction_sd)``; group shifts ``~ Normal(0, group_effect_sd)``; patient SDs
    ``~ HalfNormal(patient_sd_scale)``; pattern SDs ``~ HalfNormal(pattern_sd_scale)``;
    ``z ~ Normal(0, 1)``.
    """
    types = len(config["types"])
    patients = len(config["patient_ids"])
    patterns = len(config["pattern_ids"])
    free_intercepts = types - 1
    free_potentials = types * (types + 1) // 2 - 1
    design = potential_design(types)
    with pm.Model():
        baseline_intercept = pm.Normal(
            "baseline_intercept", 0.0, config["intercept_sd"], shape=free_intercepts
        )
        group_intercept = pm.Normal(
            "group_intercept", 0.0, config["group_effect_sd"], shape=free_intercepts
        )
        baseline_potential = pm.Normal(
            "baseline_potential", 0.0, config["interaction_sd"], shape=free_potentials
        )
        group_potential = pm.Normal(
            "group_potential", 0.0, config["group_effect_sd"], shape=free_potentials
        )
        patient_intercept_sd = pm.HalfNormal("patient_intercept_sd", config["patient_sd_scale"])
        patient_potential_sd = pm.HalfNormal("patient_potential_sd", config["patient_sd_scale"])
        pattern_intercept_sd = pm.HalfNormal("pattern_intercept_sd", config["pattern_sd_scale"])
        pattern_potential_sd = pm.HalfNormal("pattern_potential_sd", config["pattern_sd_scale"])
        patient_intercept_z = pm.Normal(
            "patient_intercept_z", 0.0, 1.0, shape=(patients, free_intercepts)
        )
        patient_potential_z = pm.Normal(
            "patient_potential_z", 0.0, 1.0, shape=(patients, free_potentials)
        )
        pattern_intercept_z = pm.Normal(
            "pattern_intercept_z", 0.0, 1.0, shape=(patterns, free_intercepts)
        )
        pattern_potential_z = pm.Normal(
            "pattern_potential_z", 0.0, 1.0, shape=(patterns, free_potentials)
        )
        pattern_group = pt.as_tensor_variable(config["pattern_groups"][:, None])
        pattern_patient = pt.as_tensor_variable(config["pattern_patients"])
        pattern_intercept_free = (
            baseline_intercept
            + pattern_group * group_intercept
            + patient_intercept_sd * patient_intercept_z[pattern_patient]
            + pattern_intercept_sd * pattern_intercept_z
        )
        pattern_potential_free = (
            baseline_potential
            + pattern_group * group_potential
            + patient_potential_sd * patient_potential_z[pattern_patient]
            + pattern_potential_sd * pattern_potential_z
        )
        pattern_intercept = pt.concatenate(
            [pt.zeros((patterns, 1)), pattern_intercept_free], axis=1
        )
        pattern_potential = pt.tensordot(
            pattern_potential_free, pt.as_tensor_variable(design), axes=1
        )
        owner = pt.as_tensor_variable(config["point_patterns"])
        owner_intercept = pattern_intercept[owner]
        owner_potential = pattern_potential[owner]
        counts = pt.as_tensor_variable(config["neighbor_counts"])
        neighbour_terms = pt.sum(owner_potential * counts[:, None, :], axis=2)
        logits = owner_intercept + neighbour_terms
        pm.Categorical("observed_type", logit_p=logits, observed=config["observed"])
        prior = pm.sample_prior_predictive(draws=500, random_seed=seed_for(config["seed"], "prior"))
        posterior = pm.sample(
            draws=config["draws"],
            tune=config["tune"],
            chains=config["chains"],
            cores=1,
            blas_cores=1,
            random_seed=[
                seed_for(config["seed"], "chain", index) for index in range(config["chains"])
            ],
            target_accept=config["target_accept"],
            nuts_sampler="pymc",
            nuts={"max_treedepth": config["maximum_tree_depth"]},
            progressbar=False,
            quiet=True,
            compute_convergence_checks=False,
        )

    draws = {name: flatten(posterior, name) for name in HIERARCHY_PARAMETERS}
    summarized = replicated_posterior(config, draws)
    prior_finite = bool(
        all(np.isfinite(np.asarray(value.values)).all() for value in prior["prior"].values())
    )
    posterior_finite = bool(
        np.isfinite(draws["baseline_intercept"]).all()
        and np.isfinite(draws["group_intercept"]).all()
        and summarized.finite
    )
    r_hat, bulk, tail = rank_convergence(posterior, HIERARCHY_PARAMETERS)
    mcse_mean, mcse_sd = mcse_extremes(posterior, HIERARCHY_PARAMETERS)
    ebfmi, divergences, depth_hits = pymc_sampler_diagnostics(posterior["sample_stats"])
    diagnostics = {
        "prior_predictive_finite": prior_finite,
        "posterior_finite": posterior_finite,
        "r_hat": r_hat,
        "ess_bulk": bulk,
        "ess_tail": tail,
        "mcse_mean": mcse_mean,
        "mcse_sd": mcse_sd,
        "minimum_ebfmi": ebfmi,
        "divergences": divergences,
        "max_tree_depth_hits": depth_hits,
        "constraints_valid": True,
        "identifiability_checks_passed": True,
    }
    complete = bool(prior_finite and posterior_finite and policy_met(config, diagnostics))
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "input_sha256": config["input_sha"],
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": summarized.completed_draws,
        },
        **summarized.sections,
        "diagnostics": diagnostics,
    }


def main() -> int:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(64 * MIB)
    config = validate_replicated_request(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=config["maximum_output_bytes"])
    return 0


if __name__ == "__main__":
    run_worker(main, "replicated conditional multitype worker failed", errors=(ValueError, OSError))
