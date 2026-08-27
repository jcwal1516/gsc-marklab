#!/usr/bin/env python3
"""Pinned NumPyro worker for patient-unit beta-binomial group regression."""

from __future__ import annotations

import hashlib
import importlib.util
import json
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
from numpyro.infer import MCMC, NUTS, Predictive


REQUEST_FORMAT = "marklab.numpyro_beta_binomial_group_regression_worker_request"
RESULT_FORMAT = "marklab.numpyro_beta_binomial_group_regression_worker_result"


class ContractError(ValueError):
    pass


def load_pymc_worker() -> Any:
    path = Path(__file__).with_name(
        "marklab_pymc_beta_binomial_group_regression_worker.py"
    )
    specification = importlib.util.spec_from_file_location(
        "marklab_pymc_beta_binomial_group_regression", path
    )
    if specification is None or specification.loader is None:
        raise ContractError("cannot load the PyMC beta-binomial group contract")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


def obj(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys - actual)}, unknown={sorted(actual - keys)}"
        )
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def validate(request: Any, lock_sha: str, worker_sha: str) -> tuple[dict[str, Any], str]:
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
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    exact(backend["name"], "numpyro", "backend.name")
    exact(backend["version"], "0.21.0", "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")
    exact(request["jax_version"], "0.11.1", "request.jax_version")
    source_sha = request["source_request_sha256"]
    if (
        not isinstance(source_sha, str)
        or len(source_sha) != 64
        or any(character not in "0123456789abcdef" for character in source_sha)
    ):
        raise ContractError("source request digest is invalid")
    pymc_path = Path(__file__).with_name(
        "marklab_pymc_beta_binomial_group_regression_worker.py"
    )
    pymc = load_pymc_worker()
    config = pymc.validate(
        request["source_request"],
        lock_sha,
        hashlib.sha256(pymc_path.read_bytes()).hexdigest(),
    )
    return config, source_sha


def seed_for(seed: int, purpose: str) -> int:
    digest = hashlib.sha256(
        f"marklab-numpyro-beta-binomial-group-v1\0{seed}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def model(
    observed_successes: jnp.ndarray | None,
    trials: jnp.ndarray,
    indicator: jnp.ndarray,
    intercept_mean: float,
    intercept_sd: float,
    group_sd: float,
    concentration_sd: float,
) -> None:
    intercept = numpyro.sample("intercept_log_odds", dist.Normal(intercept_mean, intercept_sd))
    effect = numpyro.sample("group_log_odds_effect", dist.Normal(0.0, group_sd))
    concentration = numpyro.sample("concentration", dist.HalfNormal(concentration_sd))
    probability = jax.nn.sigmoid(intercept + indicator * effect)
    numpyro.sample(
        "successes",
        dist.BetaBinomial(
            probability * concentration,
            (1.0 - probability) * concentration,
            total_count=trials,
        ),
        obs=observed_successes,
    )


def summary(values: np.ndarray) -> dict[str, float]:
    flat = values.reshape(-1)
    return {
        "mean": float(flat.mean()),
        "sd": float(flat.std(ddof=1)),
        "interval_lower": float(np.quantile(flat, 0.025)),
        "interval_upper": float(np.quantile(flat, 0.975)),
    }


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def sigmoid(values: np.ndarray) -> np.ndarray:
    return 1.0 / (1.0 + np.exp(-values))


def fit(
    config: dict[str, Any],
    request_sha: str,
    source_request_sha: str,
    lock_sha: str,
    worker_sha: str,
) -> dict[str, Any]:
    trials = jnp.asarray(config["trials"])
    successes = jnp.asarray(config["successes"])
    indicator = jnp.asarray(config["indicator"])
    arguments = {
        "trials": trials,
        "indicator": indicator,
        "intercept_mean": config["intercept_mean"],
        "intercept_sd": config["intercept_sd"],
        "group_sd": config["group_sd"],
        "concentration_sd": config["concentration_sd"],
    }
    prior = Predictive(model, num_samples=config["prior_draws"])(
        jax.random.PRNGKey(seed_for(config["seed"], "prior")),
        observed_successes=None,
        **arguments,
    )
    kernel = NUTS(
        model,
        target_accept_prob=config["target_accept"],
        max_tree_depth=config["maximum_tree_depth"],
        dense_mass=True,
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
        jax.random.PRNGKey(seed_for(config["seed"], "sample")),
        observed_successes=successes,
        **arguments,
        extra_fields=("diverging", "num_steps", "energy"),
    )
    chain_samples = {
        name: np.asarray(value, dtype=np.float64)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    flat_samples = sampler.get_samples(group_by_chain=False)
    replicated = np.asarray(
        Predictive(model, posterior_samples=flat_samples, return_sites=["successes"])(
            jax.random.PRNGKey(seed_for(config["seed"], "predictive")),
            observed_successes=None,
            **arguments,
        )["successes"],
        dtype=np.int64,
    )
    monitored = {
        "intercept_log_odds": chain_samples["intercept_log_odds"],
        "group_log_odds_effect": chain_samples["group_log_odds_effect"],
        "concentration": chain_samples["concentration"],
    }
    posterior = az.from_dict({"posterior": monitored})
    names = list(monitored)
    r_hat = float(flattened(az.rhat(posterior, var_names=names, method="rank"), names).max())
    bulk = float(flattened(az.ess(posterior, var_names=names, method="bulk"), names).min())
    tail = float(flattened(az.ess(posterior, var_names=names, method="tail"), names).min())
    mcse_mean = float(flattened(az.mcse(posterior, var_names=names, method="mean"), names).max())
    mcse_sd = float(flattened(az.mcse(posterior, var_names=names, method="sd"), names).max())
    extra = sampler.get_extra_fields(group_by_chain=True)
    energy = np.asarray(extra["energy"], dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(extra["diverging"]).sum())
    depth_hits = int(
        (np.asarray(extra["num_steps"]) >= 2 ** config["maximum_tree_depth"] - 1).sum()
    )
    intercept_draws = chain_samples["intercept_log_odds"]
    effect_draws = chain_samples["group_log_odds_effect"]
    concentration_draws = chain_samples["concentration"]
    reference_draws = sigmoid(intercept_draws)
    comparison_draws = sigmoid(intercept_draws + effect_draws)
    difference_draws = comparison_draws - reference_draws
    odds_ratio_draws = np.exp(effect_draws)
    prior_finite = bool(all(np.isfinite(np.asarray(value)).all() for value in prior.values()))
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in monitored.values())
        and np.isfinite(replicated).all()
    )
    constraints = bool(
        np.all(concentration_draws > 0.0)
        and np.all((0.0 < reference_draws) & (reference_draws < 1.0))
        and np.all((0.0 < comparison_draws) & (comparison_draws < 1.0))
        and np.isfinite(odds_ratio_draws).all()
    )
    complete = bool(
        prior_finite
        and posterior_finite
        and constraints
        and r_hat <= config["maximum_r_hat"]
        and bulk >= config["minimum_bulk_ess"]
        and tail >= config["minimum_tail_ess"]
        and ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and depth_hits <= config["maximum_tree_depth_hits"]
    )
    flat_intercept = intercept_draws.reshape(-1)
    flat_effect = effect_draws.reshape(-1)
    flat_concentration = concentration_draws.reshape(-1)
    conditional_rng = np.random.default_rng(seed_for(config["seed"], "conditional"))
    patients = []
    for index, patient_id in enumerate(config["patient_ids"]):
        group_probability_draws = sigmoid(
            flat_intercept + config["indicator"][index] * flat_effect
        )
        conditional = conditional_rng.beta(
            config["successes"][index] + group_probability_draws * flat_concentration,
            config["trials"][index]
            - config["successes"][index]
            + (1.0 - group_probability_draws) * flat_concentration,
        )
        observed = float(config["successes"][index] / config["trials"][index])
        group_mean = float(group_probability_draws.mean())
        denominator = group_mean - observed
        shrinkage = (
            0.0
            if denominator == 0.0
            else float((conditional.mean() - observed) / denominator)
        )
        patients.append(
            {
                "patient_id": patient_id,
                "group": config["groups"][index],
                "successes": int(config["successes"][index]),
                "trials": int(config["trials"][index]),
                "observed_proportion": observed,
                "posterior_probability": summary(conditional),
                "shrinkage_toward_group": shrinkage,
            }
        )
    replicated_flat = replicated.reshape(-1, replicated.shape[-1])
    reference_mask = config["indicator"] == 0.0
    comparison_mask = ~reference_mask
    observed_proportions = config["successes"] / config["trials"]
    observed_difference = float(
        observed_proportions[comparison_mask].mean()
        - observed_proportions[reference_mask].mean()
    )
    replicated_proportions = replicated_flat / config["trials"]
    replicated_differences = (
        replicated_proportions[:, comparison_mask].mean(axis=1)
        - replicated_proportions[:, reference_mask].mean(axis=1)
    )
    replicated_reference_totals = replicated_flat[:, reference_mask].sum(axis=1)
    replicated_comparison_totals = replicated_flat[:, comparison_mask].sum(axis=1)
    observed_reference_total = int(config["successes"][reference_mask].sum())
    observed_comparison_total = int(config["successes"][comparison_mask].sum())
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "numpyro",
            "version": numpyro.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
        "jax_version": jax.__version__,
        "request_sha256": request_sha,
        "source_request_sha256": source_request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            "intercept_log_odds": summary(intercept_draws),
            "group_log_odds_effect": summary(effect_draws),
            "reference_probability": summary(reference_draws),
            "comparison_probability": summary(comparison_draws),
            "probability_difference_comparison_minus_reference": summary(difference_draws),
            "odds_ratio": summary(odds_ratio_draws),
            "concentration": summary(concentration_draws),
            "overdispersion_mean": float((1.0 / (concentration_draws + 1.0)).mean()),
        },
        "patients": patients,
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
            "constraints_valid": constraints,
            "identifiability_checks_passed": True,
        },
        "posterior_predictive": {
            "observed_reference_successes": observed_reference_total,
            "observed_comparison_successes": observed_comparison_total,
            "replicated_reference_successes_mean": float(replicated_reference_totals.mean()),
            "replicated_comparison_successes_mean": float(replicated_comparison_totals.mean()),
            "probability_replicated_reference_successes_at_least_observed": float(
                np.mean(replicated_reference_totals >= observed_reference_total)
            ),
            "probability_replicated_comparison_successes_at_least_observed": float(
                np.mean(replicated_comparison_totals >= observed_comparison_total)
            ),
            "observed_group_mean_proportion_difference": observed_difference,
            "replicated_group_mean_proportion_difference_mean": float(
                replicated_differences.mean()
            ),
            "probability_replicated_difference_at_least_observed": float(
                np.mean(replicated_differences >= observed_difference)
            ),
        },
    }


def main() -> int:
    if (
        numpyro.__version__ != "0.21.0"
        or jax.__version__ != "0.11.1"
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(2 * 1024 * 1024 + 1)
    if not raw or len(raw) > 2 * 1024 * 1024:
        raise ContractError("request size is invalid")
    config, source_request_sha = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(
        config,
        hashlib.sha256(raw).hexdigest(),
        source_request_sha,
        lock_sha,
        worker_sha,
    )
    encoded = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":"))
    if len(encoded.encode()) > 2 * 1_048_576:
        raise ContractError("result exceeds output limit")
    sys.stdout.write(encoded)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(
            "Marklab NumPyro beta-binomial group worker failed: "
            f"{type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
