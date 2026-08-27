#!/usr/bin/env python3
"""Static NumPyro worker for exact gridded-LGCP cross-backend agreement."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import math
from pathlib import Path
import sys
from typing import Any

import arviz as az
import jax
jax.config.update("jax_enable_x64", True)
import jax.numpy as jnp
import numpy as np
import numpyro
import numpyro.distributions as dist
from numpyro.infer import MCMC, NUTS


REQUEST_FORMAT = "marklab.numpyro_gridded_lgcp_worker_request"
RESULT_FORMAT = "marklab.numpyro_gridded_lgcp_worker_result"
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"


class ContractError(ValueError):
    pass


def load_pymc_contract() -> Any:
    path = Path(__file__).with_name("marklab_pymc_gridded_lgcp_worker.py")
    specification = importlib.util.spec_from_file_location("marklab_pymc_lgcp_contract", path)
    if specification is None or specification.loader is None:
        raise ContractError("cannot load the PyMC gridded LGCP contract")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


def exact_object(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys - actual)}, unknown={sorted(actual - keys)}"
        )
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def validate_wrapper(
    request: Any, lock_digest: str, worker_digest: str, pymc_worker_digest: str
) -> tuple[dict[str, Any], dict[str, Any]]:
    request = exact_object(
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
    backend = exact_object(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "request.backend",
    )
    exact(backend["name"], "numpyro", "request.backend.name")
    exact(backend["version"], NUMPYRO_VERSION, "request.backend.version")
    exact(backend["python_version"], "3.12", "request.backend.python_version")
    exact(backend["environment_lock_sha256"], lock_digest, "request.backend.lock")
    exact(backend["worker_sha256"], worker_digest, "request.backend.worker")
    exact(request["jax_version"], JAX_VERSION, "request.jax_version")
    source_digest = request["source_request_sha256"]
    if (
        not isinstance(source_digest, str)
        or len(source_digest) != 64
        or any(character not in "0123456789abcdef" for character in source_digest)
    ):
        raise ContractError("request.source_request_sha256 is invalid")
    config = load_pymc_contract().validate(
        request["source_request"], lock_digest, pymc_worker_digest
    )
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    digest = hashlib.sha256(f"marklab-numpyro-lgcp-v1\0{seed}\0{purpose}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


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
    dimension = covariate.shape[0]
    intercept = numpyro.sample("intercept", dist.Normal(intercept_mean, intercept_sd))
    coefficient = numpyro.sample("coefficient", dist.Normal(coefficient_mean, coefficient_sd))
    field_raw = numpyro.sample(
        "field_raw", dist.Normal(0.0, 1.0).expand([dimension]).to_event(1)
    )
    latent_effect = numpyro.deterministic("latent_effect", cholesky @ field_raw)
    intensity = numpyro.deterministic(
        "intensity", jnp.exp(intercept + coefficient * covariate + offset + latent_effect)
    )
    expected_count = numpyro.deterministic("expected_count", cell_area * intensity)
    numpyro.sample("observed_count", dist.Poisson(expected_count), obs=counts)


def summary(draws: np.ndarray) -> dict[str, float]:
    flattened = draws.reshape(-1)
    return {
        "mean": float(flattened.mean()),
        "sd": float(flattened.std(ddof=1)),
        "interval_lower": float(np.quantile(flattened, 0.025)),
        "interval_upper": float(np.quantile(flattened, 0.975)),
    }


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


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
        covariate=jnp.asarray(config["covariate"]),
        offset=jnp.asarray(config["offset"]),
        counts=jnp.asarray(config["counts"]),
        cholesky=jnp.asarray(config["cholesky"]),
        cell_area=config["cell_area"],
        intercept_mean=config["intercept_mean"],
        intercept_sd=config["intercept_sd"],
        coefficient_mean=config["coefficient_mean"],
        coefficient_sd=config["coefficient_sd"],
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
    monitored = ["intercept", "coefficient", "field_raw"]
    r_hat = float(flattened(az.rhat(posterior, var_names=monitored, method="rank"), monitored).max())
    bulk = float(flattened(az.ess(posterior, var_names=monitored, method="bulk"), monitored).min())
    tail = float(flattened(az.ess(posterior, var_names=monitored, method="tail"), monitored).min())
    mcse_mean = float(flattened(az.mcse(posterior, var_names=monitored, method="mean"), monitored).max())
    mcse_sd = float(flattened(az.mcse(posterior, var_names=monitored, method="sd"), monitored).max())
    extra = sampler.get_extra_fields(group_by_chain=True)
    energy = np.asarray(extra["energy"], dtype=np.float64)
    minimum_ebfmi = float(
        np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1))
    )
    divergences = int(np.asarray(extra["diverging"]).sum())
    maximum_steps = 2 ** config["maximum_tree_depth"] - 1
    tree_depth_hits = int((np.asarray(extra["num_steps"]) >= maximum_steps).sum())

    predictive_rng = np.random.default_rng(seed_for(config["seed"], "posterior_predictive"))
    replicated = predictive_rng.poisson(expected_count)
    prior_rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    prior_intercept = prior_rng.normal(
        config["intercept_mean"], config["intercept_sd"], config["prior_draws"]
    )
    prior_coefficient = prior_rng.normal(
        config["coefficient_mean"], config["coefficient_sd"], config["prior_draws"]
    )
    prior_raw = prior_rng.normal(
        0.0, 1.0, (config["prior_draws"], config["covariate"].size)
    )
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
        and minimum_ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and tree_depth_hits <= config["maximum_tree_depth_hits"]
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
            "latent_effect": summary(flat_latent[:, index]),
            "intensity": summary(flat_intensity[:, index]),
            "expected_count": summary(flat_expected[:, index]),
            "pearson_residual": summary(flat_pearson[:, index]),
        }
        for index in range(config["covariate"].size)
    ]
    replicated_flat = replicated.reshape(-1, replicated.shape[-1])
    replicated_totals = replicated_flat.sum(axis=1)
    replicated_zeros = (replicated_flat == 0).sum(axis=1)
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "numpyro",
            "version": numpyro.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_digest,
            "worker_sha256": worker_digest,
        },
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
        "cells": cells,
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            "r_hat": r_hat,
            "ess_bulk": bulk,
            "ess_tail": tail,
            "mcse_mean": mcse_mean,
            "mcse_sd": mcse_sd,
            "minimum_ebfmi": minimum_ebfmi,
            "divergences": divergences,
            "max_tree_depth_hits": tree_depth_hits,
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
    if (
        numpyro.__version__ != NUMPYRO_VERSION
        or jax.__version__ != JAX_VERSION
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock_digest = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_digest = hashlib.sha256(script.read_bytes()).hexdigest()
    pymc_worker_digest = hashlib.sha256(
        script.with_name("marklab_pymc_gridded_lgcp_worker.py").read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(raw) > 16 * 1024 * 1024:
        raise ContractError("request exceeds 16 MiB")
    request_sha256 = hashlib.sha256(raw).hexdigest()
    request, config = validate_wrapper(
        json.loads(raw), lock_digest, worker_digest, pymc_worker_digest
    )
    result = fit(
        config,
        request_sha256,
        request["source_request_sha256"],
        lock_digest,
        worker_digest,
    )
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"marklab NumPyro gridded LGCP worker failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
