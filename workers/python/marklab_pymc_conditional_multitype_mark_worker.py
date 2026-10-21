#!/usr/bin/env python3
"""Pinned PyMC conditional pseudolikelihood for hard multitype marks."""

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
    free_matrices,
    free_pairs,
    policy_met,
    single_pattern_posterior,
    validate_single_request,
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

RESULT_FORMAT = "marklab.pymc_conditional_multitype_mark_result"


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-conditional-multitype-mark-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the conditional multitype pseudolikelihood by NUTS.

    Likelihood: each observed type is ``Categorical(softmax(intercept + counts @ potential.T))``
    given its neighbour type counts, independently over points (a pseudolikelihood).
    Priors: free intercepts ``~ Normal(0, intercept_sd)`` and free pair potentials
    ``~ Normal(0, interaction_sd)``; type 0 is the reference (zero intercept and zero
    self-potential) and the potential matrix is symmetric.
    """
    types = len(config["type_ids"])
    with pm.Model():
        intercept_free = pm.Normal("intercept_free", 0.0, config["intercept_sd"], shape=types - 1)
        potential_free = pm.Normal(
            "potential_free", 0.0, config["interaction_sd"], shape=len(free_pairs(types))
        )
        intercept = pt.concatenate([pt.zeros((1,)), intercept_free])
        potential = pt.zeros((types, types))
        for index, (left, right) in enumerate(free_pairs(types)):
            potential = pt.set_subtensor(potential[left, right], potential_free[index])
            potential = pt.set_subtensor(potential[right, left], potential_free[index])
        logits = intercept + pt.dot(config["neighbor_counts"], potential.T)
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
    intercept, potential = free_matrices(
        np.asarray(posterior["posterior"]["intercept_free"].values, dtype=np.float64),
        np.asarray(posterior["posterior"]["potential_free"].values, dtype=np.float64),
        types,
    )
    summarized = single_pattern_posterior(config, intercept, potential)
    monitored = ["intercept_free", "potential_free"]
    prior_finite = bool(
        all(np.isfinite(np.asarray(value.values)).all() for value in prior["prior"].values())
    )
    r_hat, bulk, tail = rank_convergence(posterior, monitored)
    mcse_mean, mcse_sd = mcse_extremes(posterior, monitored)
    ebfmi, divergences, depth_hits = pymc_sampler_diagnostics(posterior["sample_stats"])
    diagnostics = {
        "prior_predictive_finite": prior_finite,
        "posterior_finite": summarized.finite,
        "r_hat": r_hat,
        "ess_bulk": bulk,
        "ess_tail": tail,
        "mcse_mean": mcse_mean,
        "mcse_sd": mcse_sd,
        "minimum_ebfmi": ebfmi,
        "divergences": divergences,
        "max_tree_depth_hits": depth_hits,
        "constraints_valid": summarized.finite,
        "identifiability_checks_passed": summarized.identifiable,
    }
    complete = bool(prior_finite and summarized.finite and policy_met(config, diagnostics))
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
    raw = read_request(16 * MIB)
    config = validate_single_request(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=config["maximum_output_bytes"])
    return 0


if __name__ == "__main__":
    run_worker(main, "marklab conditional multitype mark failed")
