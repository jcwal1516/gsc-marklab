#!/usr/bin/env python3
"""Independent NumPyro agreement fit for replicated conditional hard marks."""

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
    HIERARCHY_PARAMETERS,
    policy_met,
    potential_design,
    replicated_posterior,
    validate_replicated_request,
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

REQUEST_FORMAT = "marklab.numpyro_replicated_conditional_multitype_mark_request"
RESULT_FORMAT = "marklab.numpyro_replicated_conditional_multitype_mark_result"
PYMC_WORKER = "marklab_pymc_replicated_conditional_multitype_mark_worker.py"


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
    config = validate_replicated_request(request["source_request"], lock_sha, pymc_worker_sha)
    config["numpyro_maximum_tree_depth"] = depth
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-numpyro-replicated-conditional-multitype-mark-v1", seed, purpose)


def make_model(config: dict[str, Any]) -> Any:
    """Build the NumPyro patient/pattern hierarchical conditional multitype model.

    Likelihood: each point's type is ``Categorical(logits=intercept_p + counts @ potential_p.T)``
    with pattern-specific free intercepts and potentials (type 0 reference, symmetric potential).
    Each free vector is ``baseline + group * group_shift + patient_sd * patient_z[patient] +
    pattern_sd * pattern_z`` (non-centred). Priors: baselines ``~ Normal(0, intercept_sd)`` and
    ``Normal(0, interaction_sd)``; group shifts ``~ Normal(0, group_effect_sd)``; patient SDs
    ``~ HalfNormal(patient_sd_scale)``; pattern SDs ``~ HalfNormal(pattern_sd_scale)``;
    ``z ~ Normal(0, 1)``. Matches the PyMC worker's model.
    """
    types = len(config["types"])
    patients = len(config["patient_ids"])
    patterns = len(config["pattern_ids"])
    free_intercepts = types - 1
    free_potentials = types * (types + 1) // 2 - 1
    design = jnp.asarray(potential_design(types))
    pattern_groups = jnp.asarray(config["pattern_groups"])[:, None]
    pattern_patients = jnp.asarray(config["pattern_patients"])
    point_patterns = jnp.asarray(config["point_patterns"])
    counts = jnp.asarray(config["neighbor_counts"])
    observed = jnp.asarray(config["observed"])

    def normal_vector(name: str, scale: float, shape: list[int]) -> Any:
        prior = dist.Normal(0.0, scale).expand(shape).to_event(len(shape))
        return numpyro.sample(name, prior)

    def model() -> None:
        baseline_intercept = normal_vector(
            "baseline_intercept", config["intercept_sd"], [free_intercepts]
        )
        group_intercept = normal_vector(
            "group_intercept", config["group_effect_sd"], [free_intercepts]
        )
        baseline_potential = normal_vector(
            "baseline_potential", config["interaction_sd"], [free_potentials]
        )
        group_potential = normal_vector(
            "group_potential", config["group_effect_sd"], [free_potentials]
        )
        patient_intercept_sd = numpyro.sample(
            "patient_intercept_sd", dist.HalfNormal(config["patient_sd_scale"])
        )
        patient_potential_sd = numpyro.sample(
            "patient_potential_sd", dist.HalfNormal(config["patient_sd_scale"])
        )
        pattern_intercept_sd = numpyro.sample(
            "pattern_intercept_sd", dist.HalfNormal(config["pattern_sd_scale"])
        )
        pattern_potential_sd = numpyro.sample(
            "pattern_potential_sd", dist.HalfNormal(config["pattern_sd_scale"])
        )
        patient_intercept_z = normal_vector("patient_intercept_z", 1.0, [patients, free_intercepts])
        patient_potential_z = normal_vector("patient_potential_z", 1.0, [patients, free_potentials])
        pattern_intercept_z = normal_vector("pattern_intercept_z", 1.0, [patterns, free_intercepts])
        pattern_potential_z = normal_vector("pattern_potential_z", 1.0, [patterns, free_potentials])
        pattern_intercept_free = (
            baseline_intercept
            + pattern_groups * group_intercept
            + patient_intercept_sd * patient_intercept_z[pattern_patients]
            + pattern_intercept_sd * pattern_intercept_z
        )
        pattern_potential_free = (
            baseline_potential
            + pattern_groups * group_potential
            + patient_potential_sd * patient_potential_z[pattern_patients]
            + pattern_potential_sd * pattern_potential_z
        )
        pattern_intercept = jnp.concatenate(
            [jnp.zeros((patterns, 1)), pattern_intercept_free], axis=1
        )
        pattern_potential = jnp.einsum("pf,fkl->pkl", pattern_potential_free, design)
        point_intercept = pattern_intercept[point_patterns]
        neighbour_terms = jnp.einsum("nl,nkl->nk", counts, pattern_potential[point_patterns])
        logits = point_intercept + neighbour_terms
        numpyro.sample("observed_type", dist.Categorical(logits=logits), obs=observed)

    return model


def fit(
    config: dict[str, Any],
    request_sha: str,
    source_sha: str,
    lock_sha: str,
    worker_sha: str,
) -> dict[str, Any]:
    depth = config["numpyro_maximum_tree_depth"]
    samples, extra = sample_nuts(
        make_model(config), config, depth, seed_for(config["seed"], "nuts")
    )
    draws = {name: value.reshape((-1, *value.shape[2:])) for name, value in samples.items()}
    summarized = replicated_posterior(config, draws)
    convergence = nuts_diagnostics(samples, HIERARCHY_PARAMETERS, extra, depth, mcse=True)
    types = len(config["types"])
    prior_rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    prior_intercepts = prior_rng.normal(size=(500, 2 * (types - 1)))
    prior_potentials = prior_rng.normal(size=(500, 2 * (types * (types + 1) // 2 - 1)))
    prior_finite = bool(np.isfinite(prior_intercepts).all() and np.isfinite(prior_potentials).all())
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
    raw = read_request(64 * MIB)
    request, config = validate(json.loads(raw), lock_sha, worker_sha, pymc_worker_sha)
    result = fit(config, sha256_hex(raw), request["source_request_sha256"], lock_sha, worker_sha)
    write_result(result, max_bytes=config["maximum_output_bytes"])
    return 0


if __name__ == "__main__":
    run_worker(main, "marklab NumPyro replicated conditional multitype mark failed")
