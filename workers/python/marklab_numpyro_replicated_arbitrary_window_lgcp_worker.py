#!/usr/bin/env python3
"""Pinned NumPyro agreement worker for the replicated exact-window patient LGCP."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import jax

jax.config.update("jax_enable_x64", True)
sys.path.append(str(Path(__file__).resolve().parent))

import arviz as az
import jax.numpy as jnp
import numpy as np
import numpyro
import numpyro.distributions as dist
from numpyro.infer import MCMC, NUTS

from marklab_worker_support import versions
from marklab_worker_support.diagnostics import (
    divergence_count,
    mcse_extremes,
    minimum_ebfmi,
    rank_convergence,
    tree_depth_hits,
)
from marklab_worker_support.posterior import summary
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
    sha256_digest,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)
from marklab_worker_support.replicated_lgcp import (
    FIXED_KERNEL_WORKER,
    HIERARCHY_PARAMETERS,
    center_within_groups,
    effect_summaries,
    expected_counts,
    fixed_kernel_latent,
    pattern_posterior_predictive,
    prior_is_finite,
    validate_request,
)

REQUEST_FORMAT = "marklab.numpyro_replicated_arbitrary_window_lgcp_request"
RESULT_FORMAT = "marklab.numpyro_replicated_arbitrary_window_lgcp_result"
MONITORED = [*HIERARCHY_PARAMETERS, "patient_raw", "pattern_raw", "field_raw"]


def validate(
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
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    exact(request["jax_version"], versions.JAX, "request.jax_version")
    sha256_digest(request["source_request_sha256"], "source request digest")
    config = validate_request(request["source_request"], lock_digest, pymc_worker_digest)
    maximum_tree_depth = request["maximum_tree_depth"]
    if (
        isinstance(maximum_tree_depth, bool)
        or not isinstance(maximum_tree_depth, int)
        or not 10 <= maximum_tree_depth <= 14
    ):
        raise ContractError("maximum_tree_depth is outside [10, 14]")
    config["numpyro_maximum_tree_depth"] = maximum_tree_depth
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-numpyro-replicated-lgcp-v1", seed, purpose)


def model(
    patient_index: jnp.ndarray,
    pattern_index: jnp.ndarray,
    pattern_patient_index: jnp.ndarray,
    group: jnp.ndarray,
    covariate: jnp.ndarray,
    weight: jnp.ndarray,
    counts: jnp.ndarray,
    cholesky: jnp.ndarray,
    priors: dict[str, float],
) -> None:
    """Fixed-kernel replicated patient LGCP (NumPyro twin of the PyMC model).

    Likelihood: node counts ``y ~ Poisson(weight * exp(eta))`` with
    ``eta = intercept + group_effect * group + covariate_effect * covariate
    + patient_effect + pattern_effect + latent``.
    Priors: ``intercept ~ Normal(intercept_mean, intercept_sd)``, group and covariate effects
    ``~ Normal(0, sd)``, ``patient_sd, pattern_sd ~ HalfNormal(scale)``.
    Parameterization: non-centred; ``patient_effect = patient_sd * z_patient``,
    ``pattern_effect = pattern_sd * (z_pattern - mean within patient)``, and
    ``latent = cholesky @ z_field`` centred within each pattern.
    """
    patient_count = group.shape[0]
    pattern_count = pattern_patient_index.shape[0]
    node_count = counts.shape[0]
    intercept = numpyro.sample(
        "intercept", dist.Normal(priors["intercept_mean"], priors["intercept_sd"])
    )
    group_effect = numpyro.sample("group_effect", dist.Normal(0.0, priors["group_effect_sd"]))
    covariate_effect = numpyro.sample(
        "covariate_effect", dist.Normal(0.0, priors["covariate_effect_sd"])
    )
    patient_sd = numpyro.sample("patient_sd", dist.HalfNormal(priors["patient_sd_scale"]))
    pattern_sd = numpyro.sample("pattern_sd", dist.HalfNormal(priors["pattern_sd_scale"]))
    patient_raw = numpyro.sample(
        "patient_raw", dist.Normal(0.0, 1.0).expand([patient_count]).to_event(1)
    )
    pattern_raw = numpyro.sample(
        "pattern_raw", dist.Normal(0.0, 1.0).expand([pattern_count]).to_event(1)
    )
    field_raw = numpyro.sample("field_raw", dist.Normal(0.0, 1.0).expand([node_count]).to_event(1))
    patient_effect = numpyro.deterministic("patient_effect", patient_sd * patient_raw)
    pattern_sums = jnp.zeros(patient_count).at[pattern_patient_index].add(pattern_raw)
    patterns_per_patient = jnp.zeros(patient_count).at[pattern_patient_index].add(1.0)
    pattern_means = pattern_sums / patterns_per_patient
    pattern_effect = numpyro.deterministic(
        "pattern_effect",
        pattern_sd * (pattern_raw - pattern_means[pattern_patient_index]),
    )
    latent_uncentered = cholesky @ field_raw
    latent_sums = jnp.zeros(pattern_count).at[pattern_index].add(latent_uncentered)
    nodes_per_pattern = jnp.zeros(pattern_count).at[pattern_index].add(1.0)
    latent_means = latent_sums / nodes_per_pattern
    latent_effect = numpyro.deterministic(
        "latent_effect", latent_uncentered - latent_means[pattern_index]
    )
    expected_count = numpyro.deterministic(
        "expected_count",
        weight
        * jnp.exp(
            intercept
            + group_effect * group[patient_index]
            + covariate_effect * covariate
            + patient_effect[patient_index]
            + pattern_effect[pattern_index]
            + latent_effect
        ),
    )
    numpyro.sample("observed_count", dist.Poisson(expected_count), obs=counts)


def model_data(config: dict[str, Any], counts: np.ndarray) -> dict[str, Any]:
    """Keyword arguments of ``model`` for the request design and the given node counts."""
    return {
        "patient_index": jnp.asarray(config["patient_index"]),
        "pattern_index": jnp.asarray(config["pattern_index"]),
        "pattern_patient_index": jnp.asarray(config["pattern_patient_index"]),
        "group": jnp.asarray(config["group"]),
        "covariate": jnp.asarray(config["covariate"]),
        "weight": jnp.asarray(config["weight"]),
        "counts": jnp.asarray(counts),
        "cholesky": jnp.asarray(config["cholesky"]),
        "priors": config["priors"],
    }


def fit(
    config: dict[str, Any],
    request_sha256: str,
    source_request_sha256: str,
    lock_digest: str,
    worker_digest: str,
) -> dict[str, Any]:
    maximum_tree_depth = config["numpyro_maximum_tree_depth"]
    kernel = NUTS(
        model, target_accept_prob=config["target_accept"], max_tree_depth=maximum_tree_depth
    )
    sampler = MCMC(
        kernel,
        num_warmup=config["tune"],
        num_samples=config["draws"],
        num_chains=config["chains"],
        chain_method="sequential",
        progress_bar=False,
    )
    sampler.run(
        jax.random.PRNGKey(seed_for(config["seed"], "nuts")),
        **model_data(config, config["counts"]),
        extra_fields=("diverging", "num_steps", "energy"),
    )
    samples = {
        name: np.asarray(value, dtype=np.float64)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    patient_effect = samples["patient_sd"][..., None] * samples["patient_raw"]
    pattern_centered = center_within_groups(
        samples["pattern_raw"].copy(),
        config["pattern_patient_index"],
        len(config["patient_ids"]),
    )
    pattern_effect = samples["pattern_sd"][..., None] * pattern_centered
    latent = fixed_kernel_latent(config, samples["field_raw"])
    expected = expected_counts(
        config,
        samples["intercept"][..., None],
        samples["group_effect"][..., None],
        samples["covariate_effect"][..., None],
        patient_effect,
        pattern_effect,
        latent,
    )
    posterior = az.from_dict({"posterior": {name: samples[name] for name in MONITORED}})
    r_hat, bulk, tail = rank_convergence(posterior, MONITORED)
    mcse_mean, mcse_sd = mcse_extremes(posterior, MONITORED)
    extra = sampler.get_extra_fields(group_by_chain=True)
    ebfmi = minimum_ebfmi(np.asarray(extra["energy"], dtype=np.float64))
    divergences = divergence_count(extra["diverging"])
    depth_hits = tree_depth_hits(extra["num_steps"], maximum_tree_depth)
    prior_finite = prior_is_finite(config)
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in samples.values())
        and np.isfinite(patient_effect).all()
        and np.isfinite(pattern_effect).all()
        and np.isfinite(latent).all()
        and np.isfinite(expected).all()
        and np.all(expected > 0.0)
    )
    policy = config["policy"]
    complete = (
        prior_finite
        and posterior_finite
        and r_hat <= policy["maximum_r_hat"]
        and bulk >= policy["minimum_bulk_ess"]
        and tail >= policy["minimum_tail_ess"]
        and ebfmi >= policy["minimum_ebfmi"]
        and divergences <= policy["maximum_divergences"]
        and depth_hits <= policy["maximum_tree_depth_hits"]
    )
    flat_expected = expected.reshape(-1, expected.shape[-1])
    predictive_rng = np.random.default_rng(seed_for(config["seed"], "posterior_predictive"))
    replicated = predictive_rng.poisson(flat_expected)
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_digest, worker_digest),
        "jax_version": jax.__version__,
        "input_sha256": config["request"]["input_sha256"],
        "request_sha256": request_sha256,
        "source_request_sha256": source_request_sha256,
        "maximum_tree_depth": maximum_tree_depth,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            name: summary(samples[name], dtype=np.float64, chains=config["chains"])
            for name in HIERARCHY_PARAMETERS
        },
        **effect_summaries(config, patient_effect, pattern_effect, latent, flat_expected),
        "pattern_posterior_predictive": pattern_posterior_predictive(config, replicated),
        "diagnostics": {
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
            "constraints_valid": posterior_finite,
            "identifiability_checks_passed": True,
        },
    }


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    pymc_worker_digest = file_sha256(Path(__file__).with_name(FIXED_KERNEL_WORKER))
    raw = read_request(16 * MIB)
    request, config = validate(json.loads(raw), lock_digest, worker_digest, pymc_worker_digest)
    result = fit(
        config, sha256_hex(raw), request["source_request_sha256"], lock_digest, worker_digest
    )
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "marklab NumPyro replicated LGCP failed")
