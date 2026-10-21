#!/usr/bin/env python3
"""Independent NumPyro agreement worker for conditional hard multitype marks."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import jax

jax.config.update("jax_enable_x64", True)
sys.path.append(str(Path(__file__).resolve().parent))

import jax.numpy as jnp
import numpy as np
import numpyro
import numpyro.distributions as dist

from marklab_worker_support import versions
from marklab_worker_support.conditional_mark import (
    free_matrices,
    free_pairs,
    policy_met,
    single_pattern_posterior,
    validate_single_request,
)
from marklab_worker_support.numpyro_nuts import nuts_diagnostics, sample_nuts
from marklab_worker_support.protocol import (
    MIB,
    backend_identity,
    derive_seed,
    exact,
    file_sha256,
    integer,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_digest,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)

REQUEST_FORMAT = "marklab.numpyro_conditional_multitype_mark_request"
RESULT_FORMAT = "marklab.numpyro_conditional_multitype_mark_result"
PYMC_WORKER = "marklab_pymc_conditional_multitype_mark_worker.py"


def validate(
    request: Any, lock_sha: str, worker_sha: str, pymc_worker_sha: str
) -> tuple[dict[str, Any], dict[str, Any]]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "jax_version",
            "source_request_sha256",
            "source_request",
            "maximum_tree_depth",
        },
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "backend",
        name="numpyro",
        version=versions.NUMPYRO,
        lock_sha256=lock_sha,
        worker_sha256=worker_sha,
    )
    exact(request["jax_version"], versions.JAX, "request.jax_version")
    sha256_digest(request["source_request_sha256"], "source_request_sha256")
    depth = integer(request["maximum_tree_depth"], "maximum_tree_depth", 10, 14)
    config = validate_single_request(request["source_request"], lock_sha, pymc_worker_sha)
    config["numpyro_maximum_tree_depth"] = depth
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-numpyro-conditional-multitype-mark-v1", seed, purpose)


def make_model(config: dict[str, Any]) -> Any:
    """Build the NumPyro conditional multitype pseudolikelihood.

    Likelihood: each observed type is ``Categorical(logits=intercept + counts @ potential.T)``
    given its neighbour type counts. Priors: free intercepts ``~ Normal(0, intercept_sd)`` and
    free pair potentials ``~ Normal(0, interaction_sd)``; type 0 is the reference and the
    potential matrix is symmetric. Matches the PyMC worker's model.
    """
    types = len(config["type_ids"])
    pairs = free_pairs(types)
    counts = jnp.asarray(config["neighbor_counts"])
    observed = jnp.asarray(config["observed"])

    def model() -> None:
        intercept_free = numpyro.sample(
            "intercept_free",
            dist.Normal(0.0, config["intercept_sd"]).expand([types - 1]).to_event(1),
        )
        potential_free = numpyro.sample(
            "potential_free",
            dist.Normal(0.0, config["interaction_sd"]).expand([len(pairs)]).to_event(1),
        )
        intercept = jnp.concatenate([jnp.zeros(1), intercept_free])
        potential = jnp.zeros((types, types))
        for index, (left, right) in enumerate(pairs):
            potential = potential.at[left, right].set(potential_free[index])
            potential = potential.at[right, left].set(potential_free[index])
        logits = intercept + counts @ potential.T
        numpyro.sample("observed_type", dist.Categorical(logits=logits), obs=observed)

    return model


def fit(
    config: dict[str, Any], request_sha: str, source_sha: str, lock_sha: str, worker_sha: str
) -> dict[str, Any]:
    depth = config["numpyro_maximum_tree_depth"]
    samples, extra = sample_nuts(
        make_model(config), config, depth, seed_for(config["seed"], "nuts")
    )
    types = len(config["type_ids"])
    intercept, potential = free_matrices(
        samples["intercept_free"], samples["potential_free"], types
    )
    summarized = single_pattern_posterior(config, intercept, potential)
    convergence = nuts_diagnostics(
        samples, ["intercept_free", "potential_free"], extra, depth, mcse=True
    )
    prior_rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    prior_intercept = prior_rng.normal(0.0, config["intercept_sd"], (500, types - 1))
    prior_potential = prior_rng.normal(0.0, config["interaction_sd"], (500, len(free_pairs(types))))
    prior_finite = bool(np.isfinite(prior_intercept).all() and np.isfinite(prior_potential).all())
    complete = bool(prior_finite and summarized.finite and policy_met(config, convergence))
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_sha, worker_sha),
        "jax_version": jax.__version__,
        "input_sha256": config["input_sha"],
        "request_sha256": request_sha,
        "source_request_sha256": source_sha,
        "maximum_tree_depth": depth,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": summarized.completed_draws,
        },
        **summarized.sections,
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": summarized.finite,
            **convergence,
            "constraints_valid": summarized.finite,
            "identifiability_checks_passed": summarized.identifiable,
        },
    }


def main() -> int:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    pymc_worker_sha = file_sha256(Path(__file__).with_name(PYMC_WORKER))
    raw = read_request(16 * MIB)
    request, config = validate(json.loads(raw), lock_sha, worker_sha, pymc_worker_sha)
    result = fit(config, sha256_hex(raw), request["source_request_sha256"], lock_sha, worker_sha)
    write_result(result, max_bytes=config["maximum_output_bytes"])
    return 0


if __name__ == "__main__":
    run_worker(main, "marklab NumPyro conditional multitype mark failed")
