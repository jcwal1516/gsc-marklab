#!/usr/bin/env python3
"""PyMC worker for a site random-effects meta-regression."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

# Configure PyTensor before PyMC is imported.
from marklab_worker_support import pytensor_config  # noqa: F401

# isort: split
import numpy as np
import pymc as pm

from marklab_worker_support import pymc_nuts, pymc_request
from marklab_worker_support.nuts_contract import (
    fit_summary,
    nuts_policy,
    nuts_sampling,
    pymc_nuts_diagnostics,
)
from marklab_worker_support.posterior import summary
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
    exact,
    integer,
    number,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)

RESULT_FORMAT = "marklab.pymc_meta_analysis_worker_result"
MODEL_KEYS = {
    "format",
    "version",
    "family",
    "covariate_name",
    "global_prior_mean",
    "global_prior_sd",
    "covariate_prior_sd",
    "heterogeneity_prior_sd",
    "observation_unit",
    "biological_unit",
    "hierarchy",
    "generated_quantities",
    "backend_capability",
    "maturity",
}
MONITORED = ["global_effect", "covariate_effect", "heterogeneity"]


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request, model = pymc_request.validate_envelope(
        request, lock_digest, worker_digest, ("sites", "new_site_covariate"), MODEL_KEYS
    )
    exact(model["family"], "bayesian_random_effects_meta_regression", "model.family")
    covariate_name = model["covariate_name"]
    if (
        not isinstance(covariate_name, str)
        or not covariate_name
        or covariate_name.strip() != covariate_name
    ):
        raise ContractError("covariate name is invalid")
    global_prior_mean = number(model["global_prior_mean"], "model.global_prior_mean")
    global_prior_sd = number(model["global_prior_sd"], "model.global_prior_sd")
    covariate_prior_sd = number(model["covariate_prior_sd"], "model.covariate_prior_sd")
    heterogeneity_prior_sd = number(model["heterogeneity_prior_sd"], "model.heterogeneity_prior_sd")
    if min(global_prior_sd, covariate_prior_sd, heterogeneity_prior_sd) <= 0.0:
        raise ContractError("all prior scales must be positive")
    exact(model["observation_unit"], "site_effect_estimate", "model.observation_unit")
    exact(model["biological_unit"], "site_or_cohort", "model.biological_unit")
    exact(model["hierarchy"], ["site"], "model.hierarchy")
    exact(
        model["generated_quantities"],
        ["site_effect", "new_site_effect"],
        "model.generated_quantities",
    )
    exact(model["backend_capability"], "nuts", "model.backend_capability")
    exact(model["maturity"], "experimental", "model.maturity")

    sites = request["sites"]
    if not isinstance(sites, list) or not 5 <= len(sites) <= 10_000:
        raise ContractError("sites must contain between 5 and 10000 entries")
    site_ids, effects, standard_errors, covariates = [], [], [], []
    for index, value in enumerate(sites):
        site = obj(value, {"site_id", "effect", "standard_error", "covariate"}, f"sites[{index}]")
        site_id = site["site_id"]
        if not isinstance(site_id, str) or not site_id or site_id.strip() != site_id:
            raise ContractError("site ID is invalid")
        if site_ids and site_id <= site_ids[-1]:
            raise ContractError("site IDs must be strictly increasing")
        effect = number(site["effect"], f"sites[{index}].effect")
        standard_error = number(site["standard_error"], f"sites[{index}].standard_error")
        covariate = number(site["covariate"], f"sites[{index}].covariate")
        if standard_error <= 0.0:
            raise ContractError("site standard errors must be positive")
        site_ids.append(site_id)
        effects.append(effect)
        standard_errors.append(standard_error)
        covariates.append(covariate)
    if all(value == covariates[0] for value in covariates):
        raise ContractError("site covariate must vary")
    new_site_covariate = number(request["new_site_covariate"], "request.new_site_covariate")

    sampling = nuts_sampling(request["sampling"], "sampling")
    resources = obj(
        request["resources"],
        {
            "maximum_observations",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    maximum_sites = integer(
        resources["maximum_observations"], "resources.maximum_observations", 1, 10_000
    )
    if len(sites) > maximum_sites:
        raise ContractError("site count exceeds resource limit")
    max_iterations = integer(
        resources["maximum_total_iterations"], "resources.maximum_total_iterations", 1, 800_000
    )
    integer(resources["maximum_output_bytes"], "resources.maximum_output_bytes", 1, MIB)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    if sampling["chains"] * (sampling["tune"] + sampling["draws"]) > max_iterations:
        raise ContractError("sampling iterations exceed resource limit")
    policy = nuts_policy(
        request["diagnostic_policy"],
        "policy",
        prior_draw_range=(100, 10_000),
        count_ceiling=sampling["chains"] * sampling["draws"],
        check_thresholds=False,
    )
    return {
        **policy,
        "global_prior_mean": global_prior_mean,
        "global_prior_sd": global_prior_sd,
        "covariate_prior_sd": covariate_prior_sd,
        "heterogeneity_prior_sd": heterogeneity_prior_sd,
        "site_ids": site_ids,
        "effects": np.asarray(effects),
        "standard_errors": np.asarray(standard_errors),
        "covariates": np.asarray(covariates),
        "new_site_covariate": new_site_covariate,
        **sampling,
    }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-meta-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the random-effects meta-regression by NUTS.

    Likelihood: site estimates ``y_s ~ Normal(mu + beta * x_s, sqrt(tau^2 + se_s^2))``. Priors:
    ``mu ~ Normal(global_prior_mean, global_prior_sd)``, ``beta ~ Normal(0,
    covariate_prior_sd)``, and heterogeneity ``tau ~ HalfNormal(heterogeneity_prior_sd)``. Site
    effects are drawn from their conditional normal posteriors given each draw, and the new-site
    effect from ``Normal(mu + beta * x_new, tau)``.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    with pm.Model(coords={"site": config["site_ids"]}):
        global_effect = pm.Normal(
            "global_effect", config["global_prior_mean"], config["global_prior_sd"]
        )
        covariate_effect = pm.Normal("covariate_effect", 0.0, config["covariate_prior_sd"])
        heterogeneity = pm.HalfNormal("heterogeneity", config["heterogeneity_prior_sd"])
        pm.Normal(
            "observed_effect",
            global_effect + config["covariates"] * covariate_effect,
            pm.math.sqrt(heterogeneity**2 + config["standard_errors"] ** 2),
            observed=config["effects"],
            dims="site",
        )
        prior, posterior, predictive = pymc_nuts.sample_prior_posterior_predictive(
            config, seed_for, "observed_effect"
        )
    posterior_arrays = {
        name: np.asarray(posterior["posterior"][name].values, dtype=np.float64)
        for name in MONITORED
    }
    population_means = (
        posterior_arrays["global_effect"][..., None]
        + posterior_arrays["covariate_effect"][..., None] * config["covariates"]
    )
    tau_squared = posterior_arrays["heterogeneity"][..., None] ** 2
    se_squared = config["standard_errors"] ** 2
    conditional_variance = tau_squared * se_squared / (tau_squared + se_squared)
    conditional_mean = (se_squared * population_means + tau_squared * config["effects"]) / (
        tau_squared + se_squared
    )
    rng = np.random.default_rng(seed_for(config["seed"], "conditional_effects"))
    posterior_arrays["site_effect"] = rng.normal(conditional_mean, np.sqrt(conditional_variance))
    new_site_location = (
        posterior_arrays["global_effect"]
        + config["new_site_covariate"] * posterior_arrays["covariate_effect"]
    )
    posterior_arrays["new_site_effect"] = rng.normal(
        new_site_location, posterior_arrays["heterogeneity"]
    )
    predictive_draws = np.asarray(
        predictive["posterior_predictive"]["observed_effect"].values, dtype=np.float64
    )
    prior_finite = bool(
        all(np.isfinite(prior["prior"][name].values).all() for name in MONITORED)
        and np.isfinite(prior["prior_predictive"]["observed_effect"].values).all()
    )
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in posterior_arrays.values())
        and np.isfinite(predictive_draws).all()
    )
    convergence = pymc_nuts_diagnostics(posterior, MONITORED)
    constraints = bool(np.all(posterior_arrays["heterogeneity"] >= 0.0))
    site_summaries = []
    for index, site_id in enumerate(config["site_ids"]):
        draws = posterior_arrays["site_effect"][..., index]
        site_summaries.append(
            {
                "site_id": site_id,
                "observed_effect": float(config["effects"][index]),
                "standard_error": float(config["standard_errors"][index]),
                "covariate": float(config["covariates"][index]),
                "posterior_mean": float(draws.mean()),
                "posterior_sd": float(draws.std(ddof=1)),
                "interval_lower": float(np.quantile(draws, 0.025)),
                "interval_upper": float(np.quantile(draws, 0.975)),
            }
        )
    replicated_means = predictive_draws.mean(axis=-1)
    replicated_sds = predictive_draws.std(axis=-1, ddof=1)
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        **fit_summary(convergence, config, prior_finite, posterior_finite, constraints),
        "posterior": {name: summary(posterior_arrays[name], flatten=False) for name in MONITORED},
        "site_effects": site_summaries,
        "new_site_prediction": {
            "covariate": config["new_site_covariate"],
            **summary(posterior_arrays["new_site_effect"], flatten=False),
        },
        "posterior_predictive": {
            "observed_effect_mean": float(config["effects"].mean()),
            "replicated_effect_mean": float(replicated_means.mean()),
            "replicated_effect_mean_sd": float(replicated_means.std(ddof=1)),
            "observed_effect_sd": float(config["effects"].std(ddof=1)),
            "replicated_effect_sd_mean": float(replicated_sds.mean()),
        },
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(16 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    write_result(fit(config, sha256_hex(raw), lock_sha, worker_sha))


if __name__ == "__main__":
    run_worker(main, "marklab PyMC worker failed")
