#!/usr/bin/env python3
"""NumPyro agreement worker for the inferred-kernel replicated patient LGCP."""

from __future__ import annotations

import json
import sys
from collections.abc import Callable
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
    backend_identity,
    derive_seed,
    file_sha256,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)
from marklab_worker_support.replicated_lgcp import (
    FIXED_KERNEL_WORKER,
    HIERARCHY_PARAMETERS,
    INFERRED_KERNEL_WORKER,
    _fit_is_complete,
    _validate_numpyro_envelope,
    center_within_groups,
    effect_summaries,
    inferred_kernel_prior_is_finite,
    matern32,
    pattern_distances,
    validate_inferred_kernel_request,
)

REQUEST_FORMAT = "marklab.numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_request"
RESULT_FORMAT = "marklab.numpyro_replicated_arbitrary_window_lgcp_inferred_kernel_result"
KERNEL_PARAMETERS = ("field_amplitude", "field_length_scale_um")
MONITORED = [
    *HIERARCHY_PARAMETERS,
    *KERNEL_PARAMETERS,
    "patient_raw",
    "pattern_raw",
    "field_raw",
]


def validate(
    request: Any,
    lock_digest: str,
    worker_digest: str,
    pymc_worker_digest: str,
    fixed_worker_digest: str,
) -> tuple[dict[str, Any], dict[str, Any]]:
    return _validate_numpyro_envelope(
        request,
        REQUEST_FORMAT,
        lock_digest,
        worker_digest,
        lambda source: validate_inferred_kernel_request(
            source, lock_digest, pymc_worker_digest, fixed_worker_digest
        ),
    )


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-numpyro-replicated-inferred-kernel-v1", seed, purpose)


def make_model(config: dict[str, Any]) -> Callable[[], None]:
    """Build the inferred-kernel replicated patient LGCP for the counts in ``config``.

    Likelihood: node counts ``y ~ Poisson(weight * exp(eta))`` with
    ``eta = intercept + group_effect * group + covariate_effect * covariate
    + patient_effect + pattern_effect + latent``.
    Priors: ``intercept ~ Normal(intercept_mean, intercept_sd)``, group and covariate effects
    ``~ Normal(0, sd)``, ``patient_sd, pattern_sd ~ HalfNormal(scale)``,
    ``field_amplitude ~ HalfNormal(amplitude_scale)``, and
    ``field_length_scale_um ~ HalfNormal(length_scale_scale)``.
    Parameterization: non-centred hierarchy (pattern effects centred within patient); each
    pattern's latent block is ``chol(K + jitter I) z_field`` centred within the pattern, with
    ``K`` the Matérn-3/2 covariance of the node distances. Nodes of one pattern are contiguous.
    """
    patient_index = jnp.asarray(config["patient_index"])
    pattern_index = jnp.asarray(config["pattern_index"])
    pattern_patient_index = jnp.asarray(config["pattern_patient_index"])
    group = jnp.asarray(config["group"])
    covariate = jnp.asarray(config["covariate"])
    weight = jnp.asarray(config["weight"])
    counts = jnp.asarray(config["counts"])
    priors = config["priors"]
    patient_count = len(config["patient_ids"])
    slices = []
    distances = []
    start = 0
    for selected, distance in pattern_distances(config):
        distances.append(jnp.asarray(distance))
        slices.append((start, start + len(selected)))
        start += len(selected)

    def model() -> None:
        intercept = numpyro.sample(
            "intercept", dist.Normal(priors["intercept_mean"], priors["intercept_sd"])
        )
        group_effect = numpyro.sample("group_effect", dist.Normal(0.0, priors["group_effect_sd"]))
        covariate_effect = numpyro.sample(
            "covariate_effect", dist.Normal(0.0, priors["covariate_effect_sd"])
        )
        patient_sd = numpyro.sample("patient_sd", dist.HalfNormal(priors["patient_sd_scale"]))
        pattern_sd = numpyro.sample("pattern_sd", dist.HalfNormal(priors["pattern_sd_scale"]))
        amplitude = numpyro.sample(
            "field_amplitude", dist.HalfNormal(config["field_amplitude_scale"])
        )
        length = numpyro.sample(
            "field_length_scale_um", dist.HalfNormal(config["field_length_scale_scale_um"])
        )
        standard_normal = dist.Normal(0.0, 1.0)
        patient_raw = numpyro.sample(
            "patient_raw", standard_normal.expand([patient_count]).to_event(1)
        )
        pattern_raw = numpyro.sample(
            "pattern_raw", standard_normal.expand([len(config["pattern_ids"])]).to_event(1)
        )
        field_raw = numpyro.sample(
            "field_raw", standard_normal.expand([len(config["counts"])]).to_event(1)
        )
        patient_effect = numpyro.deterministic("patient_effect", patient_sd * patient_raw)
        sums = jnp.zeros(patient_count).at[pattern_patient_index].add(pattern_raw)
        sizes = jnp.zeros(patient_count).at[pattern_patient_index].add(1.0)
        pattern_effect = numpyro.deterministic(
            "pattern_effect", pattern_sd * (pattern_raw - (sums / sizes)[pattern_patient_index])
        )
        blocks = []
        for (begin, end), distance in zip(slices, distances, strict=True):
            covariance = matern32(distance, amplitude, length, jnp.exp)
            covariance += config["kernel_jitter"] * jnp.eye(end - begin)
            block = jnp.linalg.cholesky(covariance) @ field_raw[begin:end]
            blocks.append(block - jnp.mean(block))
        latent = numpyro.deterministic("latent_effect", jnp.concatenate(blocks))
        expected = numpyro.deterministic(
            "expected_count",
            weight
            * jnp.exp(
                intercept
                + group_effect * group[patient_index]
                + covariate_effect * covariate
                + patient_effect[patient_index]
                + pattern_effect[pattern_index]
                + latent
            ),
        )
        numpyro.sample("observed_count", dist.Poisson(expected), obs=counts)

    return model


def fit(
    config: dict[str, Any], request_sha: str, source_sha: str, lock_sha: str, worker_sha: str
) -> dict[str, Any]:
    maximum_tree_depth = config["numpyro_maximum_tree_depth"]
    kernel = NUTS(
        make_model(config),
        target_accept_prob=config["target_accept"],
        max_tree_depth=maximum_tree_depth,
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
        extra_fields=("diverging", "num_steps", "energy"),
    )
    samples = {
        name: np.asarray(value, dtype=np.float64)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    posterior = az.from_dict({"posterior": samples})
    r_hat, bulk, tail = rank_convergence(posterior, MONITORED)
    mcse_mean, mcse_sd = mcse_extremes(posterior, MONITORED)
    extra = sampler.get_extra_fields(group_by_chain=True)
    ebfmi = minimum_ebfmi(np.asarray(extra["energy"], dtype=np.float64))
    divergences = divergence_count(extra["diverging"])
    depth_hits = tree_depth_hits(extra["num_steps"], maximum_tree_depth)
    patient_effect = samples["patient_sd"][..., None] * samples["patient_raw"]
    pattern_centered = center_within_groups(
        samples["pattern_raw"].copy(),
        config["pattern_patient_index"],
        len(config["patient_ids"]),
    )
    pattern_effect = samples["pattern_sd"][..., None] * pattern_centered
    policy = config["policy"]
    prior_finite = inferred_kernel_prior_is_finite(config)
    finite = all(np.isfinite(value).all() for value in samples.values())
    convergence = {
        "r_hat": r_hat,
        "ess_bulk": bulk,
        "ess_tail": tail,
        "minimum_ebfmi": ebfmi,
        "divergences": divergences,
        "max_tree_depth_hits": depth_hits,
    }
    complete = _fit_is_complete(prior_finite, finite, policy, convergence)
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_sha, worker_sha),
        "jax_version": jax.__version__,
        "request_sha256": request_sha,
        "source_request_sha256": source_sha,
        "input_sha256": config["request"]["input_sha256"],
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
            for name in (*HIERARCHY_PARAMETERS, *KERNEL_PARAMETERS)
        },
        **effect_summaries(
            config,
            patient_effect,
            pattern_effect,
            samples["latent_effect"],
            samples["expected_count"],
        ),
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": finite,
            **convergence,
            "mcse_mean": mcse_mean,
            "mcse_sd": mcse_sd,
            "constraints_valid": finite,
            "identifiability_checks_passed": True,
        },
    }


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock, worker = worker_digests(__file__)
    pymc_worker = file_sha256(Path(__file__).with_name(INFERRED_KERNEL_WORKER))
    fixed_worker = file_sha256(Path(__file__).with_name(FIXED_KERNEL_WORKER))
    raw = read_request(16 * MIB)
    request, config = validate(json.loads(raw), lock, worker, pymc_worker, fixed_worker)
    write_result(fit(config, sha256_hex(raw), request["source_request_sha256"], lock, worker))


if __name__ == "__main__":
    run_worker(main, "marklab NumPyro inferred-kernel LGCP failed")
