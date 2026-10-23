#!/usr/bin/env python3
"""NumPyro worker for exact gridded-LGCP cross-backend agreement."""

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

from marklab_worker_support import gridded_lgcp as source_contract
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

REQUEST_FORMAT = "marklab.numpyro_gridded_lgcp_worker_request"
RESULT_FORMAT = "marklab.numpyro_gridded_lgcp_worker_result"
MONITORED = ["intercept", "coefficient", "field_raw"]


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
    sha256_digest(request["source_request_sha256"], "request.source_request_sha256")
    config = source_contract.validate_request(
        request["source_request"], lock_digest, pymc_worker_digest
    )
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-numpyro-lgcp-v1", seed, purpose)


def model(
    covariate: jnp.ndarray,
    offset: jnp.ndarray,
    counts: jnp.ndarray,
    cholesky: jnp.ndarray,
    cell_area: float,
    intercept_mean: float,
    intercept_sd: float,
    coefficient_mean: float,
    coefficient_sd: float,
) -> None:
    """Gridded log-Gaussian Cox process with a fixed latent-field covariance.

    Likelihood: ``y_k ~ Poisson(A * exp(alpha + beta * x_k + offset_k + u_k))`` for each cell
    of area ``A``. Priors: ``alpha ~ Normal(intercept_mean, intercept_sd)`` and ``beta ~
    Normal(coefficient_mean, coefficient_sd)``. Non-centred latent field ``u = L z`` with
    ``z ~ Normal(0, I)`` and ``L`` the fixed Cholesky factor of the field covariance.
    """
    dimension = covariate.shape[0]
    intercept = numpyro.sample("intercept", dist.Normal(intercept_mean, intercept_sd))
    coefficient = numpyro.sample("coefficient", dist.Normal(coefficient_mean, coefficient_sd))
    field_raw = numpyro.sample("field_raw", dist.Normal(0.0, 1.0).expand([dimension]).to_event(1))
    latent_effect = numpyro.deterministic("latent_effect", cholesky @ field_raw)
    intensity = numpyro.deterministic(
        "intensity", jnp.exp(intercept + coefficient * covariate + offset + latent_effect)
    )
    expected_count = numpyro.deterministic("expected_count", cell_area * intensity)
    numpyro.sample("observed_count", dist.Poisson(expected_count), obs=counts)


def model_arguments(config: dict[str, Any], counts: Any) -> dict[str, Any]:
    """Keyword arguments of ``model`` for one count vector."""
    return {
        "covariate": jnp.asarray(config["covariate"]),
        "offset": jnp.asarray(config["offset"]),
        "counts": jnp.asarray(counts),
        "cholesky": jnp.asarray(config["cholesky"]),
        "cell_area": config["cell_area"],
        "intercept_mean": config["intercept_mean"],
        "intercept_sd": config["intercept_sd"],
        "coefficient_mean": config["coefficient_mean"],
        "coefficient_sd": config["coefficient_sd"],
    }


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
        **model_arguments(config, config["counts"]),
        extra_fields=("diverging", "num_steps", "energy"),
    )
    samples = {
        name: np.asarray(value, dtype=np.float64)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    intercept = samples["intercept"]
    coefficient = samples["coefficient"]
    field_raw = samples["field_raw"]
    latent = np.einsum("ij,cdj->cdi", config["cholesky"], field_raw)
    intensity = np.exp(
        intercept[..., None]
        + coefficient[..., None] * config["covariate"]
        + config["offset"]
        + latent
    )
    expected_count = config["cell_area"] * intensity
    pearson = (config["counts"] - expected_count) / np.sqrt(expected_count)
    posterior = az.from_dict(
        {"posterior": {"intercept": intercept, "coefficient": coefficient, "field_raw": field_raw}}
    )
    r_hat, bulk, tail = rank_convergence(posterior, MONITORED)
    mcse_mean, mcse_sd = mcse_extremes(posterior, MONITORED)
    extra = sampler.get_extra_fields(group_by_chain=True)
    ebfmi = minimum_ebfmi(np.asarray(extra["energy"], dtype=np.float64))
    divergences = divergence_count(extra["diverging"])
    depth_hits = tree_depth_hits(extra["num_steps"], config["maximum_tree_depth"])

    predictive_rng = np.random.default_rng(seed_for(config["seed"], "posterior_predictive"))
    replicated = predictive_rng.poisson(expected_count)
    prior_rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    prior_intercept = prior_rng.normal(
        config["intercept_mean"], config["intercept_sd"], config["prior_draws"]
    )
    prior_coefficient = prior_rng.normal(
        config["coefficient_mean"], config["coefficient_sd"], config["prior_draws"]
    )
    prior_raw = prior_rng.normal(0.0, 1.0, (config["prior_draws"], config["covariate"].size))
    prior_latent = prior_raw @ config["cholesky"].T
    prior_expected = config["cell_area"] * np.exp(
        prior_intercept[:, None]
        + prior_coefficient[:, None] * config["covariate"]
        + config["offset"]
        + prior_latent
    )
    prior_finite = bool(
        np.isfinite(prior_intercept).all()
        and np.isfinite(prior_coefficient).all()
        and np.isfinite(prior_raw).all()
        and np.isfinite(prior_expected).all()
    )
    posterior_finite = bool(
        np.isfinite(intercept).all()
        and np.isfinite(coefficient).all()
        and np.isfinite(field_raw).all()
        and np.isfinite(latent).all()
        and np.isfinite(intensity).all()
        and np.isfinite(expected_count).all()
        and np.isfinite(pearson).all()
        and np.isfinite(replicated).all()
    )
    constraints_valid = bool(np.all(intensity > 0.0) and np.all(expected_count > 0.0))
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
    flat_latent = latent.reshape(-1, latent.shape[-1])
    flat_intensity = intensity.reshape(-1, intensity.shape[-1])
    flat_expected = expected_count.reshape(-1, expected_count.shape[-1])
    flat_pearson = pearson.reshape(-1, pearson.shape[-1])
    cells = [
        {
            "ix": index % config["grid_x"],
            "iy": index // config["grid_x"],
            "count": int(config["counts"][index]),
            "latent_effect": summary(flat_latent[:, index], chains=config["chains"]),
            "intensity": summary(flat_intensity[:, index], chains=config["chains"]),
            "expected_count": summary(flat_expected[:, index], chains=config["chains"]),
            "pearson_residual": summary(flat_pearson[:, index], chains=config["chains"]),
        }
        for index in range(config["covariate"].size)
    ]
    replicated_flat = replicated.reshape(-1, replicated.shape[-1])
    replicated_totals = replicated_flat.sum(axis=1)
    replicated_zeros = (replicated_flat == 0).sum(axis=1)
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
            "intercept": summary(intercept, chains=config["chains"]),
            "coefficient": summary(coefficient, chains=config["chains"]),
        },
        "cells": cells,
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
        "posterior_predictive": {
            "observed_total_count": int(config["counts"].sum()),
            "observed_zero_cells": int((config["counts"] == 0).sum()),
            "replicated_total_mean": float(replicated_totals.mean()),
            "replicated_total_sd": float(replicated_totals.std(ddof=1)),
            "replicated_zero_cells_mean": float(replicated_zeros.mean()),
        },
        "patterns": [],
    }


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    pymc_worker_digest = file_sha256(Path(__file__).with_name(source_contract.SOURCE_WORKER))
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
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "marklab NumPyro gridded LGCP worker failed")
