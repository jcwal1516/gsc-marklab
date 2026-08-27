#!/usr/bin/env python3
"""Pinned NumPyro worker for the additive patient group/gender count model."""

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


REQUEST_FORMAT = "marklab.numpyro_beta_binomial_group_gender_worker_request"
RESULT_FORMAT = "marklab.numpyro_beta_binomial_group_gender_worker_result"


class ContractError(ValueError):
    pass


def load_pymc_worker() -> Any:
    path = Path(__file__).with_name(
        "marklab_pymc_beta_binomial_group_gender_regression_worker.py"
    )
    specification = importlib.util.spec_from_file_location("marklab_group_gender", path)
    if specification is None or specification.loader is None:
        raise ContractError("cannot load the PyMC group/gender contract")
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
        {"format", "version", "backend", "jax_version", "source_request_sha256", "source_request"},
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
    if not isinstance(source_sha, str) or len(source_sha) != 64 or any(
        character not in "0123456789abcdef" for character in source_sha
    ):
        raise ContractError("source request digest is invalid")
    pymc_path = Path(__file__).with_name(
        "marklab_pymc_beta_binomial_group_gender_regression_worker.py"
    )
    pymc = load_pymc_worker()
    config = pymc.validate(
        request["source_request"], lock_sha, hashlib.sha256(pymc_path.read_bytes()).hexdigest()
    )
    return config, source_sha


def seed_for(seed: int, purpose: str) -> int:
    digest = hashlib.sha256(
        f"marklab-numpyro-beta-binomial-group-gender-v1\0{seed}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def model(
    observed_successes: jnp.ndarray | None,
    trials: jnp.ndarray,
    group_indicator: jnp.ndarray,
    gender_indicator: jnp.ndarray,
    intercept_mean: float,
    intercept_sd: float,
    group_sd: float,
    gender_sd: float,
    concentration_sd: float,
) -> None:
    intercept = numpyro.sample("intercept_log_odds", dist.Normal(intercept_mean, intercept_sd))
    group_effect = numpyro.sample("group_log_odds_effect", dist.Normal(0.0, group_sd))
    gender_effect = numpyro.sample("gender_log_odds_effect", dist.Normal(0.0, gender_sd))
    concentration = numpyro.sample("concentration", dist.HalfNormal(concentration_sd))
    probability = jax.nn.sigmoid(
        intercept + group_indicator * group_effect + gender_indicator * gender_effect
    )
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


def fit(config: dict[str, Any], request_sha: str, source_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    arguments = {
        "trials": jnp.asarray(config["trials"]),
        "group_indicator": jnp.asarray(config["group_indicator"]),
        "gender_indicator": jnp.asarray(config["gender_indicator"]),
        "intercept_mean": config["intercept_mean"],
        "intercept_sd": config["intercept_sd"],
        "group_sd": config["group_sd"],
        "gender_sd": config["gender_sd"],
        "concentration_sd": config["concentration_sd"],
    }
    prior = Predictive(model, num_samples=config["prior_draws"])(
        jax.random.PRNGKey(seed_for(config["seed"], "prior")),
        observed_successes=None,
        **arguments,
    )
    sampler = MCMC(
        NUTS(
            model,
            target_accept_prob=config["target_accept"],
            max_tree_depth=config["maximum_tree_depth"],
            dense_mass=True,
        ),
        num_warmup=config["tune"],
        num_samples=config["draws"],
        num_chains=config["chains"],
        chain_method="sequential",
        progress_bar=False,
    )
    sampler.run(
        jax.random.PRNGKey(seed_for(config["seed"], "sample")),
        observed_successes=jnp.asarray(config["successes"]),
        **arguments,
        extra_fields=("diverging", "num_steps", "energy"),
    )
    chain_samples = {
        name: np.asarray(value, dtype=np.float64)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    replicated = np.asarray(
        Predictive(
            model,
            posterior_samples=sampler.get_samples(group_by_chain=False),
            return_sites=["successes"],
        )(
            jax.random.PRNGKey(seed_for(config["seed"], "predictive")),
            observed_successes=None,
            **arguments,
        )["successes"],
        dtype=np.int64,
    )
    names = ["intercept_log_odds", "group_log_odds_effect", "gender_log_odds_effect", "concentration"]
    posterior = az.from_dict({"posterior": {name: chain_samples[name] for name in names}})
    r_hat = float(flattened(az.rhat(posterior, var_names=names, method="rank"), names).max())
    bulk = float(flattened(az.ess(posterior, var_names=names, method="bulk"), names).min())
    tail = float(flattened(az.ess(posterior, var_names=names, method="tail"), names).min())
    mcse_mean = float(flattened(az.mcse(posterior, var_names=names, method="mean"), names).max())
    mcse_sd = float(flattened(az.mcse(posterior, var_names=names, method="sd"), names).max())
    extra = sampler.get_extra_fields(group_by_chain=True)
    energy = np.asarray(extra["energy"], dtype=np.float64)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)))
    divergences = int(np.asarray(extra["diverging"]).sum())
    depth_hits = int((np.asarray(extra["num_steps"]) >= 2 ** config["maximum_tree_depth"] - 1).sum())
    intercept = chain_samples["intercept_log_odds"]
    group_effect = chain_samples["group_log_odds_effect"]
    gender_effect = chain_samples["gender_log_odds_effect"]
    concentration = chain_samples["concentration"]
    p00 = sigmoid(intercept)
    p10 = sigmoid(intercept + group_effect)
    p01 = sigmoid(intercept + gender_effect)
    p11 = sigmoid(intercept + group_effect + gender_effect)
    gender_weight = float(config["gender_indicator"].mean())
    marginal_reference = (1.0 - gender_weight) * p00 + gender_weight * p01
    marginal_comparison = (1.0 - gender_weight) * p10 + gender_weight * p11
    marginal_difference = marginal_comparison - marginal_reference
    group_odds_ratio = np.exp(group_effect)
    gender_odds_ratio = np.exp(gender_effect)
    prior_finite = bool(all(np.isfinite(np.asarray(value)).all() for value in prior.values()))
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in chain_samples.values())
        and np.isfinite(replicated).all()
    )
    constraints = bool(
        np.all(concentration > 0.0)
        and all(np.all((0.0 < value) & (value < 1.0)) for value in [p00, p10, p01, p11])
        and np.isfinite(group_odds_ratio).all()
        and np.isfinite(gender_odds_ratio).all()
    )
    complete = bool(
        prior_finite and posterior_finite and constraints
        and r_hat <= config["maximum_r_hat"]
        and bulk >= config["minimum_bulk_ess"]
        and tail >= config["minimum_tail_ess"]
        and ebfmi >= config["minimum_ebfmi"]
        and divergences <= config["maximum_divergences"]
        and depth_hits <= config["maximum_tree_depth_hits"]
    )
    flat_intercept = intercept.reshape(-1)
    flat_group = group_effect.reshape(-1)
    flat_gender = gender_effect.reshape(-1)
    flat_concentration = concentration.reshape(-1)
    rng = np.random.default_rng(seed_for(config["seed"], "conditional"))
    patients = []
    for index, patient_id in enumerate(config["patient_ids"]):
        linear_probability = sigmoid(
            flat_intercept
            + config["group_indicator"][index] * flat_group
            + config["gender_indicator"][index] * flat_gender
        )
        conditional = rng.beta(
            config["successes"][index] + linear_probability * flat_concentration,
            config["trials"][index] - config["successes"][index]
            + (1.0 - linear_probability) * flat_concentration,
        )
        observed = float(config["successes"][index] / config["trials"][index])
        denominator = float(linear_probability.mean()) - observed
        patients.append({
            "patient_id": patient_id,
            "group": config["groups"][index],
            "gender": config["genders"][index],
            "successes": int(config["successes"][index]),
            "trials": int(config["trials"][index]),
            "observed_proportion": observed,
            "posterior_probability": summary(conditional),
            "shrinkage_toward_linear_predictor": 0.0 if denominator == 0.0 else float((conditional.mean() - observed) / denominator),
        })
    replicated_flat = replicated.reshape(-1, replicated.shape[-1])
    group_reference = config["group_indicator"] == 0.0
    group_comparison = ~group_reference
    gender_reference = config["gender_indicator"] == 0.0
    gender_comparison = ~gender_reference
    observed_proportions = config["successes"] / config["trials"]
    replicated_proportions = replicated_flat / config["trials"]
    observed_group_difference = float(observed_proportions[group_comparison].mean() - observed_proportions[group_reference].mean())
    replicated_group_difference = replicated_proportions[:, group_comparison].mean(axis=1) - replicated_proportions[:, group_reference].mean(axis=1)
    observed_gender_difference = float(observed_proportions[gender_comparison].mean() - observed_proportions[gender_reference].mean())
    replicated_gender_difference = replicated_proportions[:, gender_comparison].mean(axis=1) - replicated_proportions[:, gender_reference].mean(axis=1)
    replicated_totals = replicated_flat.sum(axis=1)
    observed_total = int(config["successes"].sum())
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "numpyro", "version": numpyro.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha,
        },
        "jax_version": jax.__version__,
        "request_sha256": request_sha,
        "source_request_sha256": source_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"], "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"], "completed_draws": config["chains"] * config["draws"],
        },
        "posterior": {
            "intercept_log_odds": summary(intercept),
            "group_log_odds_effect": summary(group_effect),
            "gender_log_odds_effect": summary(gender_effect),
            "reference_group_reference_gender_probability": summary(p00),
            "comparison_group_reference_gender_probability": summary(p10),
            "reference_group_comparison_gender_probability": summary(p01),
            "comparison_group_comparison_gender_probability": summary(p11),
            "reference_gender_probability_difference": summary(p10 - p00),
            "comparison_gender_probability_difference": summary(p11 - p01),
            "marginal_reference_group_probability": summary(marginal_reference),
            "marginal_comparison_group_probability": summary(marginal_comparison),
            "marginal_probability_difference_comparison_minus_reference": summary(marginal_difference),
            "group_odds_ratio": summary(group_odds_ratio),
            "gender_odds_ratio": summary(gender_odds_ratio),
            "concentration": summary(concentration),
            "overdispersion_mean": float((1.0 / (concentration + 1.0)).mean()),
        },
        "patients": patients,
        "diagnostics": {
            "prior_predictive_finite": prior_finite, "posterior_finite": posterior_finite,
            "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail,
            "mcse_mean": mcse_mean, "mcse_sd": mcse_sd, "minimum_ebfmi": ebfmi,
            "divergences": divergences, "max_tree_depth_hits": depth_hits,
            "constraints_valid": constraints, "identifiability_checks_passed": True,
        },
        "posterior_predictive": {
            "observed_total_successes": observed_total,
            "replicated_total_successes_mean": float(replicated_totals.mean()),
            "probability_replicated_total_successes_at_least_observed": float(np.mean(replicated_totals >= observed_total)),
            "observed_group_mean_proportion_difference": observed_group_difference,
            "replicated_group_mean_proportion_difference_mean": float(replicated_group_difference.mean()),
            "probability_replicated_group_difference_at_least_observed": float(np.mean(replicated_group_difference >= observed_group_difference)),
            "observed_gender_mean_proportion_difference": observed_gender_difference,
            "replicated_gender_mean_proportion_difference_mean": float(replicated_gender_difference.mean()),
            "probability_replicated_gender_difference_at_least_observed": float(np.mean(replicated_gender_difference >= observed_gender_difference)),
        },
    }


def main() -> int:
    if numpyro.__version__ != "0.21.0" or jax.__version__ != "0.11.1" or sys.version_info[:2] != (3, 12):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(2 * 1024 * 1024 + 1)
    if not raw or len(raw) > 2 * 1024 * 1024:
        raise ContractError("request size is invalid")
    config, source_sha = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, hashlib.sha256(raw).hexdigest(), source_sha, lock_sha, worker_sha)
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
            f"Marklab NumPyro group/gender worker failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
