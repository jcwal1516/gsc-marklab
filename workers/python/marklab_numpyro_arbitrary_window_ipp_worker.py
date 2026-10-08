#!/usr/bin/env python3
"""Static NumPyro worker for weighted exact-window IPP agreement."""

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


REQUEST_FORMAT = "marklab.numpyro_arbitrary_window_ipp_request"
RESULT_FORMAT = "marklab.numpyro_arbitrary_window_ipp_result"
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"


class ContractError(ValueError):
    pass


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


def load_pymc_contract() -> Any:
    path = Path(__file__).with_name("marklab_pymc_inhomogeneous_poisson_worker.py")
    specification = importlib.util.spec_from_file_location("marklab_pymc_ipp_contract", path)
    if specification is None or specification.loader is None:
        raise ContractError("cannot load the PyMC IPP contract")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


def validate(
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
        "backend",
    )
    exact(backend["name"], "numpyro", "backend.name")
    exact(backend["version"], NUMPYRO_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    exact(request["jax_version"], JAX_VERSION, "request.jax_version")
    source_digest = request["source_request_sha256"]
    if (
        not isinstance(source_digest, str)
        or len(source_digest) != 64
        or any(character not in "0123456789abcdef" for character in source_digest)
    ):
        raise ContractError("source request digest is invalid")
    config = load_pymc_contract().validate(
        request["source_request"], lock_digest, pymc_worker_digest
    )
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    digest = hashlib.sha256(
        f"marklab-numpyro-arbitrary-window-ipp-v1\0{seed}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


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
    intercept = numpyro.sample("intercept", dist.Normal(intercept_mean, intercept_sd))
    coefficient = numpyro.sample("coefficient", dist.Normal(coefficient_mean, coefficient_sd))
    event_eta = intercept + coefficient * event_covariate + event_offset
    grid_eta = intercept + coefficient * grid_covariate + grid_offset
    numpyro.factor(
        "point_process_likelihood",
        jnp.sum(event_eta) - cell_area * jnp.sum(jnp.exp(grid_eta)),
    )


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
    posterior = az.from_dict(
        {"posterior": {"intercept": intercept, "coefficient": coefficient}}
    )
    monitored = ["intercept", "coefficient"]
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
    depth_hits = int((np.asarray(extra["num_steps"]) >= maximum_steps).sum())
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
        and minimum_ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and depth_hits <= config["maximum_tree_depth_hits"]
    )
    totals = expected.reshape(-1, expected.shape[-1]).sum(axis=1)
    replicated_totals = replicated.reshape(-1, replicated.shape[-1]).sum(axis=1)
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
            "max_tree_depth_hits": depth_hits,
            "constraints_valid": constraints_valid,
            "identifiability_checks_passed": True,
        },
        "total_expected_count_mean": float(totals.mean()),
        "replicated_total_mean": float(replicated_totals.mean()),
        "replicated_total_sd": float(replicated_totals.std(ddof=1)),
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
        script.with_name("marklab_pymc_inhomogeneous_poisson_worker.py").read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(raw) > 16 * 1024 * 1024:
        raise ContractError("request exceeds 16 MiB")
    request_sha256 = hashlib.sha256(raw).hexdigest()
    request, config = validate(
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
            f"marklab NumPyro arbitrary-window IPP worker failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
