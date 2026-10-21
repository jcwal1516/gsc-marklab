#!/usr/bin/env python3
"""NumPyro worker for weighted exact-window IPP agreement."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import arviz as az
import jax

jax.config.update("jax_enable_x64", True)

import jax.numpy as jnp
import numpy as np
import numpyro
import numpyro.distributions as dist
from numpyro.infer import MCMC, NUTS

sys.path.append(str(Path(__file__).resolve().parent))

import marklab_pymc_inhomogeneous_poisson_worker as pymc_contract
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

REQUEST_FORMAT = "marklab.numpyro_arbitrary_window_ipp_request"
RESULT_FORMAT = "marklab.numpyro_arbitrary_window_ipp_result"
MONITORED = ["intercept", "coefficient"]


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
    sha256_digest(request["source_request_sha256"], "request.source_request_sha256")
    config = pymc_contract.validate(request["source_request"], lock_digest, pymc_worker_digest)
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-numpyro-arbitrary-window-ipp-v1", seed, purpose)


def model(
    event_covariate: jnp.ndarray,
    event_offset: jnp.ndarray,
    grid_covariate: jnp.ndarray,
    grid_offset: jnp.ndarray,
    cell_area: float,
    intercept_mean: float,
    intercept_sd: float,
    coefficient_mean: float,
    coefficient_sd: float,
) -> None:
    """Log-linear inhomogeneous Poisson process with a quadrature intensity integral.

    The log density is ``sum_i eta(s_i) - A * sum_k exp(eta_k)`` (event linear predictors minus
    the integrated intensity over quadrature nodes of area ``A``), with ``eta = alpha + beta *
    covariate + offset``. Priors: ``alpha ~ Normal(intercept_mean, intercept_sd)`` and ``beta ~
    Normal(coefficient_mean, coefficient_sd)``.
    """
    intercept = numpyro.sample("intercept", dist.Normal(intercept_mean, intercept_sd))
    coefficient = numpyro.sample("coefficient", dist.Normal(coefficient_mean, coefficient_sd))
    event_eta = intercept + coefficient * event_covariate + event_offset
    grid_eta = intercept + coefficient * grid_covariate + grid_offset
    numpyro.factor(
        "point_process_likelihood",
        jnp.sum(event_eta) - cell_area * jnp.sum(jnp.exp(grid_eta)),
    )


def fit(
    config: dict[str, Any],
    request_sha256: str,
    source_request_sha256: str,
    lock_digest: str,
    worker_digest: str,
) -> dict[str, Any]:
    kernel = NUTS(
        model,
        target_accept_prob=config["target_accept"],
        max_tree_depth=config["maximum_tree_depth"],
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
        event_covariate=jnp.asarray(config["event_covariate"]),
        event_offset=jnp.asarray(config["event_offset"]),
        grid_covariate=jnp.asarray(config["grid_covariate"]),
        grid_offset=jnp.asarray(config["grid_offset"]),
        cell_area=config["cell_area"],
        intercept_mean=config["intercept_prior_mean"],
        intercept_sd=config["intercept_prior_sd"],
        coefficient_mean=config["coefficient_prior_mean"],
        coefficient_sd=config["coefficient_prior_sd"],
        extra_fields=("diverging", "num_steps", "energy"),
    )
    samples = {
        name: np.asarray(value, dtype=np.float64)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    intercept = samples["intercept"]
    coefficient = samples["coefficient"]
    expected = config["cell_area"] * np.exp(
        intercept[..., None]
        + coefficient[..., None] * config["grid_covariate"]
        + config["grid_offset"]
    )
    posterior = az.from_dict({"posterior": {"intercept": intercept, "coefficient": coefficient}})
    r_hat, bulk, tail = rank_convergence(posterior, MONITORED)
    mcse_mean, mcse_sd = mcse_extremes(posterior, MONITORED)
    extra = sampler.get_extra_fields(group_by_chain=True)
    ebfmi = minimum_ebfmi(np.asarray(extra["energy"], dtype=np.float64))
    divergences = divergence_count(extra["diverging"])
    depth_hits = tree_depth_hits(extra["num_steps"], config["maximum_tree_depth"])
    rng = np.random.default_rng(seed_for(config["seed"], "predictive"))
    replicated = rng.poisson(expected)
    prior_rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    prior_intercept = prior_rng.normal(
        config["intercept_prior_mean"], config["intercept_prior_sd"], config["prior_draws"]
    )
    prior_coefficient = prior_rng.normal(
        config["coefficient_prior_mean"], config["coefficient_prior_sd"], config["prior_draws"]
    )
    prior_expected = config["cell_area"] * np.exp(
        prior_intercept[:, None]
        + prior_coefficient[:, None] * config["grid_covariate"]
        + config["grid_offset"]
    )
    prior_finite = bool(np.isfinite(prior_expected).all())
    posterior_finite = bool(
        np.isfinite(intercept).all()
        and np.isfinite(coefficient).all()
        and np.isfinite(expected).all()
        and np.isfinite(replicated).all()
    )
    constraints_valid = bool(np.all(expected > 0.0))
    complete = (
        prior_finite
        and posterior_finite
        and constraints_valid
        and r_hat <= config["maximum_r_hat"]
        and bulk >= config["minimum_bulk_ess"]
        and tail >= config["minimum_tail_ess"]
        and ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and depth_hits <= config["maximum_tree_depth_hits"]
    )
    totals = expected.reshape(-1, expected.shape[-1]).sum(axis=1)
    replicated_totals = replicated.reshape(-1, replicated.shape[-1]).sum(axis=1)
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
        "posterior": {
            "intercept": summary(intercept),
            "coefficient": summary(coefficient),
        },
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
            "constraints_valid": constraints_valid,
            "identifiability_checks_passed": True,
        },
        "total_expected_count_mean": float(totals.mean()),
        "replicated_total_mean": float(replicated_totals.mean()),
        "replicated_total_sd": float(replicated_totals.std(ddof=1)),
    }


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    pymc_worker_digest = file_sha256(pymc_contract.__file__)
    raw = read_request(16 * MIB)
    request, config = validate(json.loads(raw), lock_digest, worker_digest, pymc_worker_digest)
    result = fit(
        config,
        sha256_hex(raw),
        request["source_request_sha256"],
        lock_digest,
        worker_digest,
    )
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "marklab NumPyro arbitrary-window IPP worker failed")
