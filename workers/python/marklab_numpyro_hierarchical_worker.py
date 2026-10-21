#!/usr/bin/env python3
"""NumPyro worker for cross-backend Gaussian patient-hierarchy agreement."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import jax

jax.config.update("jax_enable_x64", True)

import jax.numpy as jnp
import numpy as np
import numpyro
import numpyro.distributions as dist
from numpyro.infer import NUTS

sys.path.append(str(Path(__file__).resolve().parent))

from marklab_worker_support import versions
from marklab_worker_support.normal_models import (
    hierarchy_summaries,
    patient_index,
    validate_hierarchy_request,
)
from marklab_worker_support.numpyro_nuts import nuts_diagnostics, run_chains
from marklab_worker_support.nuts_contract import convergence_passes
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
    exact,
    file_sha256,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)

REQUEST_FORMAT = "marklab.numpyro_hierarchical_worker_request"
RESULT_FORMAT = "marklab.numpyro_hierarchical_worker_result"
PYMC_WORKER = "marklab_pymc_hierarchical_worker.py"


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-numpyro-hierarchy-v1", seed, purpose)


def validate_wrapper(
    request: Any, lock_digest: str, worker_digest: str, pymc_worker_digest: str
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
        },
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "request.backend",
        name="numpyro",
        version=versions.NUMPYRO,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    exact(request["jax_version"], versions.JAX, "request.jax_version")
    source_digest = request["source_request_sha256"]
    if not isinstance(source_digest, str) or len(source_digest) != 64:
        raise ContractError("source request digest is invalid")
    config = validate_hierarchy_request(request["source_request"], lock_digest, pymc_worker_digest)
    return request, config


def model(
    patient_index: jnp.ndarray,
    observations: jnp.ndarray,
    patient_count: int,
    global_prior_mean: float,
    global_prior_sd: float,
    between_prior_sd: float,
    known_sigma: float,
) -> None:
    """Gaussian patient varying-intercept hierarchy, identical to the PyMC worker's model.

    ``global_mean ~ Normal(global_prior_mean, global_prior_sd)``,
    ``between_patient_sd ~ HalfNormal(between_prior_sd)``, non-centred patient means
    ``patient_mean = global_mean + between_patient_sd * patient_z`` with
    ``patient_z ~ Normal(0, 1)``, and ``y ~ Normal(patient_mean[patient_index], known_sigma)``.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    global_mean = numpyro.sample("global_mean", dist.Normal(global_prior_mean, global_prior_sd))
    between_patient_sd = numpyro.sample("between_patient_sd", dist.HalfNormal(between_prior_sd))
    patient_z = numpyro.sample(
        "patient_z", dist.Normal(0.0, 1.0).expand([patient_count]).to_event(1)
    )
    patient_mean = numpyro.deterministic(
        "patient_mean", global_mean + between_patient_sd * patient_z
    )
    numpyro.sample("y", dist.Normal(patient_mean[patient_index], known_sigma), obs=observations)


def fit(
    config: dict[str, Any],
    request_sha256: str,
    source_request_sha256: str,
    lock_digest: str,
    worker_digest: str,
) -> dict[str, Any]:
    """Fit ``model`` by NUTS and summarize it on the PyMC worker's result scale.

    Prior-predictive finiteness and posterior-predictive draws are simulated with NumPy from
    the same generative model.
    """
    observation_patient = patient_index(config["patient_values"], np.int32)
    observations = np.concatenate(config["patient_values"]).astype(np.float64)
    kernel = NUTS(
        model,
        target_accept_prob=config["target_accept"],
        max_tree_depth=config["maximum_tree_depth"],
    )
    sampler, samples = run_chains(
        kernel,
        config,
        jax.random.PRNGKey(seed_for(config["seed"], "nuts")),
        {
            "patient_index": jnp.asarray(observation_patient),
            "observations": jnp.asarray(observations),
            "patient_count": len(config["patient_ids"]),
            "global_prior_mean": config["global_prior_mean"],
            "global_prior_sd": config["global_prior_sd"],
            "between_prior_sd": config["between_prior_sd"],
            "known_sigma": config["known_sigma"],
        },
    )
    monitored = ["global_mean", "between_patient_sd", "patient_mean"]
    diagnostics = nuts_diagnostics(
        samples,
        monitored,
        sampler.get_extra_fields(group_by_chain=True),
        config["maximum_tree_depth"],
        mcse=True,
    )

    global_draws = samples["global_mean"]
    between_draws = samples["between_patient_sd"]
    patient_draws = samples["patient_mean"]
    rng = np.random.default_rng(seed_for(config["seed"], "posterior_predictive"))
    predictive_draws = rng.normal(patient_draws[..., observation_patient], config["known_sigma"])

    prior_draws = config["prior_draws"]
    prior_rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    prior_global = prior_rng.normal(
        config["global_prior_mean"], config["global_prior_sd"], prior_draws
    )
    prior_between = np.abs(prior_rng.normal(0.0, config["between_prior_sd"], prior_draws))
    prior_patient_z = prior_rng.normal(0.0, 1.0, (prior_draws, len(config["patient_ids"])))
    prior_patient_mean = prior_global[:, None] + prior_between[:, None] * prior_patient_z
    prior_y = prior_rng.normal(prior_patient_mean[:, observation_patient], config["known_sigma"])
    prior_finite = bool(
        np.isfinite(prior_global).all()
        and np.isfinite(prior_between).all()
        and np.isfinite(prior_y).all()
    )
    posterior_finite = bool(
        all(np.isfinite(samples[name]).all() for name in monitored)
        and np.isfinite(predictive_draws).all()
    )
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
        "version": 1,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_digest, worker_digest),
        "jax_version": jax.__version__,
        "request_sha256": request_sha256,
        "source_request_sha256": source_request_sha256,
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
            "constraints_valid": True,
            "identifiability_checks_passed": True,
        },
    }


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    pymc_worker_digest = file_sha256(Path(__file__).with_name(PYMC_WORKER))
    raw = read_request(16 * MIB)
    request, config = validate_wrapper(
        json.loads(raw), lock_digest, worker_digest, pymc_worker_digest
    )
    result = fit(
        config,
        sha256_hex(raw),
        request["source_request_sha256"],
        lock_digest,
        worker_digest,
    )
    write_result(result, max_bytes=config["maximum_output_bytes"], newline=False)


if __name__ == "__main__":
    run_worker(main, "Marklab NumPyro hierarchy worker failed")
