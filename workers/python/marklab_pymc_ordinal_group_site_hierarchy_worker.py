#!/usr/bin/env python3
"""Static PyMC worker for a patient ordinal group model with site-varying effects."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any
import unicodedata

import numpy as np
import pymc as pm

PYMC_VERSION = "6.3.0"


class ContractError(ValueError):
    pass


def obj(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys-actual)}, unknown={sorted(actual-keys)}"
        )
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def number(value: Any, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise ContractError(f"{path} must be finite")
    return result


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low}, {high}]")
    return value


def valid_name(value: Any) -> bool:
    return (
        isinstance(value, str)
        and bool(value)
        and len(value) <= 128
        and value == value.strip()
        and not any(unicodedata.category(character) == "Cc" for character in value)
    )


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    request = obj(
        request,
        {"format", "version", "backend", "model", "site_ids", "patients", "sampling", "resources", "diagnostic_policy"},
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    exact(request["version"], 1, "request.version")
    backend = obj(request["backend"], {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"}, "backend")
    exact(backend["name"], "pymc", "backend.name")
    exact(backend["version"], PYMC_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")
    model = obj(
        request["model"],
        {
            "format", "version", "family", "reference_group", "comparison_group", "ordered_levels",
            "cutpoint_prior", "cutpoint_prior_sd", "group_effect_prior", "group_effect_prior_sd",
            "site_intercept_prior", "site_intercept_sd_prior_sd", "site_group_slope_prior",
            "site_group_slope_sd_prior_sd", "constraints", "likelihood",
            "group_probability_standardization", "observation_unit", "biological_unit",
            "backend_capability", "maturity",
        },
        "model",
    )
    for path, expected in [
        ("format", "marklab.bayesian_model_ir"),
        ("version", 1),
        ("family", "patient_proportional_odds_site_varying_group_regression"),
        ("cutpoint_prior", "ordered_normal"),
        ("group_effect_prior", "normal_global_log_odds_difference"),
        ("site_intercept_prior", "sum_zero_noncentered_normal"),
        ("site_group_slope_prior", "sum_zero_noncentered_normal"),
        ("constraints", "site_intercepts_and_site_group_slope_deviations_sum_zero"),
        ("likelihood", "ordered_logistic_patient_outcome"),
        ("group_probability_standardization", "equal_weight_sites"),
        ("observation_unit", "one_complete_ordered_outcome_per_patient"),
        ("biological_unit", "patient"),
        ("backend_capability", "nuts"),
        ("maturity", "experimental"),
    ]:
        exact(model[path], expected, f"model.{path}")
    reference_group = model["reference_group"]
    comparison_group = model["comparison_group"]
    levels = model["ordered_levels"]
    site_ids = request["site_ids"]
    if (
        not valid_name(reference_group)
        or not valid_name(comparison_group)
        or reference_group == comparison_group
        or not isinstance(levels, list)
        or not 2 <= len(levels) <= 8
        or not all(valid_name(level) for level in levels)
        or len(set(levels)) != len(levels)
        or not isinstance(site_ids, list)
        or not 8 <= len(site_ids) <= 64
        or site_ids != sorted(site_ids)
        or not all(valid_name(site) for site in site_ids)
        or len(set(site_ids)) != len(site_ids)
    ):
        raise ContractError("model groups, levels, or sites are invalid")
    scales = [
        number(model["cutpoint_prior_sd"], "model.cutpoint_sd"),
        number(model["group_effect_prior_sd"], "model.group_sd"),
        number(model["site_intercept_sd_prior_sd"], "model.site_intercept_sd"),
        number(model["site_group_slope_sd_prior_sd"], "model.site_slope_sd"),
    ]
    if min(scales) <= 0.0:
        raise ContractError("prior scales must be positive")
    patients = request["patients"]
    if not isinstance(patients, list) or not 32 <= len(patients) <= 1024:
        raise ContractError("patient count is invalid")
    site_lookup = {site: index for index, site in enumerate(site_ids)}
    patient_ids, groups, outcomes, site_index = [], [], [], []
    counts = np.zeros((len(site_ids), 2), dtype=np.int64)
    for index, raw in enumerate(patients):
        row = obj(raw, {"patient_id", "site_id", "group", "outcome_code"}, f"patients[{index}]")
        if not valid_name(row["patient_id"]) or row["site_id"] not in site_lookup or row["group"] not in {reference_group, comparison_group}:
            raise ContractError("patient identity is invalid")
        patient_ids.append(row["patient_id"])
        groups.append(row["group"])
        outcomes.append(integer(row["outcome_code"], "patient.outcome", 0, len(levels)-1))
        site_index.append(site_lookup[row["site_id"]])
        counts[site_index[-1], int(row["group"] == comparison_group)] += 1
    if patient_ids != sorted(patient_ids) or len(set(patient_ids)) != len(patient_ids) or np.any(counts < 2) or set(outcomes) != set(range(len(levels))):
        raise ContractError("patient order, site support, or level support is invalid")
    sampling = obj(request["sampling"], {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"}, "sampling")
    chains = integer(sampling["chains"], "sampling.chains", 2, 8)
    tune = integer(sampling["tune_per_chain"], "sampling.tune", 100, 100000)
    draws = integer(sampling["draws_per_chain"], "sampling.draws", 100, 100000)
    target_accept = number(sampling["target_accept"], "sampling.target_accept")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64-1)
    resources = obj(request["resources"], {"maximum_patients", "maximum_sites", "maximum_levels", "maximum_total_iterations", "maximum_output_bytes", "timeout_seconds"}, "resources")
    if len(patients) > integer(resources["maximum_patients"], "resources.patients", 32, 1024) or len(site_ids) > integer(resources["maximum_sites"], "resources.sites", 8, 64) or len(levels) > integer(resources["maximum_levels"], "resources.levels", 2, 8) or chains*(tune+draws) > integer(resources["maximum_total_iterations"], "resources.iterations", 1, 400000):
        raise ContractError("resource limit exceeded")
    output_limit = integer(resources["maximum_output_bytes"], "resources.output", 1, 2*1048576)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3600)
    policy = obj(request["diagnostic_policy"], {"prior_predictive_draws", "maximum_r_hat", "minimum_bulk_ess", "minimum_tail_ess", "minimum_ebfmi", "maximum_divergences", "maximum_tree_depth_hits", "maximum_tree_depth"}, "policy")
    return {
        "reference_group": reference_group, "comparison_group": comparison_group,
        "levels": levels, "site_ids": site_ids, "groups": groups,
        "indicator": np.asarray([group == comparison_group for group in groups], dtype=np.float64),
        "outcomes": np.asarray(outcomes, dtype=np.int64), "site_index": np.asarray(site_index, dtype=np.int64),
        "cutpoint_sd": scales[0], "group_sd": scales[1], "site_intercept_sd_prior": scales[2], "site_slope_sd_prior": scales[3],
        "chains": chains, "tune": tune, "draws": draws, "target_accept": target_accept, "seed": seed, "output_limit": output_limit,
        "prior_draws": integer(policy["prior_predictive_draws"], "policy.prior", 1, 100000),
        "maximum_r_hat": number(policy["maximum_r_hat"], "policy.rhat"), "minimum_bulk_ess": number(policy["minimum_bulk_ess"], "policy.bulk"),
        "minimum_tail_ess": number(policy["minimum_tail_ess"], "policy.tail"), "minimum_ebfmi": number(policy["minimum_ebfmi"], "policy.ebfmi"),
        "maximum_divergences": integer(policy["maximum_divergences"], "policy.div", 0, 2**63-1),
        "maximum_tree_depth_hits": integer(policy["maximum_tree_depth_hits"], "policy.depth_hits", 0, 2**63-1),
        "maximum_tree_depth": integer(policy["maximum_tree_depth"], "policy.depth", 1, 32),
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    digest = hashlib.sha256(f"marklab-pymc-ordinal-site-v1\0{seed}\0{purpose}\0{index}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


def summary(values: np.ndarray) -> dict[str, float]:
    flat = values.reshape(-1)
    return {"mean": float(flat.mean()), "sd": float(flat.std(ddof=1)), "interval_lower": float(np.quantile(flat, .025)), "interval_upper": float(np.quantile(flat, .975))}


def sigmoid(values: np.ndarray) -> np.ndarray:
    return 1.0 / (1.0 + np.exp(-np.clip(values, -700.0, 700.0)))


def probabilities(cutpoints: np.ndarray, eta: np.ndarray) -> np.ndarray:
    cumulative = sigmoid(cutpoints - eta[..., None])
    return np.diff(np.concatenate([np.zeros((*cumulative.shape[:-1], 1)), cumulative, np.ones((*cumulative.shape[:-1], 1))], axis=-1), axis=-1)


def tree_values(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    initial = np.linspace(-1.5, 1.5, len(config["levels"])-1)
    with pm.Model():
        cutpoints = pm.Normal("cutpoints", mu=initial, sigma=config["cutpoint_sd"], shape=len(initial), transform=pm.distributions.transforms.ordered, initval=initial)
        global_effect = pm.Normal("global_group_log_odds_effect", 0.0, config["group_sd"])
        intercept_sd = pm.HalfNormal("site_intercept_sd", config["site_intercept_sd_prior"])
        slope_sd = pm.HalfNormal("site_group_slope_sd", config["site_slope_sd_prior"])
        intercept_raw = pm.Normal("site_intercept_raw", 0.0, 1.0, shape=len(config["site_ids"]))
        slope_raw = pm.Normal("site_group_slope_raw", 0.0, 1.0, shape=len(config["site_ids"]))
        site_intercept = pm.Deterministic("site_intercept", (intercept_raw-intercept_raw.mean())*intercept_sd)
        site_slope = pm.Deterministic("site_group_slope_deviation", (slope_raw-slope_raw.mean())*slope_sd)
        eta = site_intercept[config["site_index"]] + config["indicator"] * (global_effect + site_slope[config["site_index"]])
        pm.OrderedLogistic("outcome", eta=eta, cutpoints=cutpoints, observed=config["outcomes"])
        posterior = pm.sample(draws=config["draws"], tune=config["tune"], chains=config["chains"], cores=1, blas_cores=1, random_seed=[seed_for(config["seed"], "chain", i) for i in range(config["chains"])], target_accept=config["target_accept"], nuts_sampler="pymc", nuts={"max_treedepth": config["maximum_tree_depth"]}, progressbar=False, quiet=True, compute_convergence_checks=False)
        predictive = pm.sample_posterior_predictive(posterior, var_names=["outcome"], random_seed=seed_for(config["seed"], "predictive"), progressbar=False)
    draws = {name: np.asarray(posterior["posterior"][name].values, dtype=np.float64) for name in ["cutpoints", "global_group_log_odds_effect", "site_intercept_sd", "site_group_slope_sd", "site_intercept", "site_group_slope_deviation"]}
    reference_probabilities = probabilities(draws["cutpoints"][:, :, None, :], draws["site_intercept"])
    comparison_eta = draws["site_intercept"] + draws["global_group_log_odds_effect"][..., None] + draws["site_group_slope_deviation"]
    comparison_probabilities = probabilities(draws["cutpoints"][:, :, None, :], comparison_eta)
    reference_marginal = reference_probabilities.mean(axis=2)
    comparison_marginal = comparison_probabilities.mean(axis=2)
    codes = np.arange(len(config["levels"]), dtype=np.float64)
    replicated = np.asarray(predictive["posterior_predictive"]["outcome"].values, dtype=np.int64)
    monitored = ["cutpoints", "global_group_log_odds_effect", "site_intercept_sd", "site_group_slope_sd", "site_intercept_raw", "site_group_slope_raw"]
    prior_rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    prior_finite = bool(np.isfinite(prior_rng.normal(size=(config["prior_draws"], 4))).all())
    posterior_finite = bool(all(np.isfinite(value).all() for value in draws.values()) and np.isfinite(replicated).all())
    r_hat = float(tree_values(pm.stats.rhat(posterior, var_names=monitored, method="rank"), monitored).max())
    bulk = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="bulk"), monitored).min())
    tail = float(tree_values(pm.stats.ess(posterior, var_names=monitored, method="tail"), monitored).min())
    mcse_mean = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="mean"), monitored).max())
    mcse_sd = float(tree_values(pm.stats.mcse(posterior, var_names=monitored, method="sd"), monitored).max())
    energy = np.asarray(posterior["sample_stats"]["energy"].values)
    ebfmi = float(np.min(np.mean(np.diff(energy, axis=1)**2, axis=1)/np.var(energy, axis=1)))
    divergences = int(np.asarray(posterior["sample_stats"]["diverging"].values).sum())
    depth_hits = int(np.asarray(posterior["sample_stats"]["reached_max_treedepth"].values).sum())
    constraints = bool(np.all(draws["site_intercept_sd"] > 0) and np.all(draws["site_group_slope_sd"] > 0) and np.all(np.diff(draws["cutpoints"], axis=-1) > 0) and np.allclose(draws["site_intercept"].sum(axis=-1), 0, atol=1e-10) and np.allclose(draws["site_group_slope_deviation"].sum(axis=-1), 0, atol=1e-10))
    complete = bool(prior_finite and posterior_finite and constraints and r_hat <= config["maximum_r_hat"] and bulk >= config["minimum_bulk_ess"] and tail >= config["minimum_tail_ess"] and ebfmi >= config["minimum_ebfmi"] and divergences <= config["maximum_divergences"] and depth_hits <= config["maximum_tree_depth_hits"])
    flat_replicated = replicated.reshape(-1, replicated.shape[-1])
    reference_mask = config["indicator"] == 0
    comparison_mask = ~reference_mask
    flat_reference = reference_marginal.reshape(-1, len(config["levels"]))
    flat_comparison = comparison_marginal.reshape(-1, len(config["levels"]))
    levels, ppc = [], []
    for index, level in enumerate(config["levels"]):
        ref, comp = flat_reference[:, index], flat_comparison[:, index]
        obs_ref = float(np.mean(config["outcomes"][reference_mask] == index)); obs_comp = float(np.mean(config["outcomes"][comparison_mask] == index))
        rep_ref = np.mean(flat_replicated[:, reference_mask] == index, axis=1); rep_comp = np.mean(flat_replicated[:, comparison_mask] == index, axis=1)
        observed_error = abs(obs_ref-ref.mean()) + abs(obs_comp-comp.mean())
        levels.append({"level": level, "reference_probability": summary(ref), "comparison_probability": summary(comp), "difference_comparison_minus_reference": summary(comp-ref)})
        ppc.append({"level": level, "observed_reference_proportion": obs_ref, "observed_comparison_proportion": obs_comp, "replicated_reference_proportion_mean": float(rep_ref.mean()), "replicated_comparison_proportion_mean": float(rep_comp.mean()), "probability_absolute_replication_error_at_least_observed": float(np.mean(np.abs(rep_ref-ref)+np.abs(rep_comp-comp) >= observed_error))})
    return {
        "format": "marklab.pymc_ordinal_group_site_hierarchy_worker_result", "version": 1,
        "backend": {"name": "pymc", "version": pm.__version__, "python_version": f"{sys.version_info.major}.{sys.version_info.minor}", "environment_lock_sha256": lock_sha, "worker_sha256": worker_sha},
        "request_sha256": request_sha, "fit_state": "complete" if complete else "nonconverged",
        "sampling": {"chains": config["chains"], "tune_per_chain": config["tune"], "draws_per_chain": config["draws"], "completed_draws": config["chains"]*config["draws"]},
        "posterior": {
            "global_group_log_odds_effect": summary(draws["global_group_log_odds_effect"]), "site_intercept_sd": summary(draws["site_intercept_sd"]), "site_group_slope_sd": summary(draws["site_group_slope_sd"]),
            "cutpoints": [{"lower_level": config["levels"][i], "upper_level": config["levels"][i+1], **summary(draws["cutpoints"][..., i])} for i in range(len(config["levels"])-1)],
            "sites": [{"site_id": site, "intercept": summary(draws["site_intercept"][..., i]), "group_slope_deviation": summary(draws["site_group_slope_deviation"][..., i])} for i, site in enumerate(config["site_ids"])],
            "levels": levels, "reference_expected_code": summary((reference_marginal*codes).sum(axis=-1)), "comparison_expected_code": summary((comparison_marginal*codes).sum(axis=-1)),
        },
        "diagnostics": {"prior_predictive_finite": prior_finite, "posterior_finite": posterior_finite, "r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail, "mcse_mean": mcse_mean, "mcse_sd": mcse_sd, "minimum_ebfmi": ebfmi, "divergences": divergences, "max_tree_depth_hits": depth_hits, "constraints_valid": constraints, "identifiability_checks_passed": True},
        "posterior_predictive": {"levels": ppc},
    }


def main() -> None:
    if pm.__version__ != PYMC_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("PyMC or Python version drift")
    script = Path(__file__); lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest(); worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(2*1048576+1)
    if not raw or len(raw) > 2*1048576: raise ContractError("request size is invalid")
    config = validate(json.loads(raw), lock_sha, worker_sha)
    encoded = json.dumps(fit(config, hashlib.sha256(raw).hexdigest(), lock_sha, worker_sha), allow_nan=False, sort_keys=True, separators=(",", ":"))
    if len(encoded.encode()) > config["output_limit"]: raise ContractError("result exceeds output limit")
    sys.stdout.write(encoded + "\n")


if __name__ == "__main__":
    try: main()
    except Exception as error:
        print(f"Marklab PyMC ordinal site hierarchy worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
