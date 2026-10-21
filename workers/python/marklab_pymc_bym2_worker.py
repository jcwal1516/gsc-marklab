#!/usr/bin/env python3
"""PyMC worker for Marklab's scaled-ICAR Poisson BYM2 disease-mapping model."""

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

from marklab_worker_support import areal, pymc_nuts, versions
from marklab_worker_support.nuts_contract import (
    convergence_passes,
    nuts_policy,
    nuts_sampling,
    pymc_nuts_diagnostics,
    sampling_record,
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
    validate_backend,
    worker_digests,
    write_result,
)

REQUEST_KEYS = {
    "format",
    "version",
    "backend",
    "model",
    "region_ids",
    "weights_digest_sha256",
    "scaled_icar_transform",
    "rank_deficiency",
    "original_typical_marginal_variance",
    "scaled_typical_marginal_variance",
    "components",
    "counts",
    "expected",
    "design",
    "predictor_names",
    "sampling",
    "resources",
    "diagnostic_policy",
}
MODEL_KEYS = {
    "format",
    "version",
    "family",
    "likelihood",
    "offset",
    "reparameterization",
    "icar_scaling",
    "intercept_prior_mean",
    "intercept_prior_sd",
    "coefficient_prior_sd",
    "total_sd_prior",
    "phi_alpha",
    "phi_beta",
    "backend_capability",
    "maturity",
}


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(request, REQUEST_KEYS, "request")
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    validate_backend(
        request["backend"],
        "request.backend",
        name="pymc",
        version=versions.PYMC,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )

    model = obj(request["model"], MODEL_KEYS, "request.model")
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "poisson_bym2_disease_mapping", "model.family")
    exact(model["likelihood"], "poisson_log_link", "model.likelihood")
    exact(model["offset"], "log_expected", "model.offset")
    exact(
        model["reparameterization"],
        "sigma_times_sqrt_phi_icar_plus_sqrt_one_minus_phi_iid",
        "model.reparameterization",
    )
    exact(
        model["icar_scaling"],
        "geometric_mean_generalized_marginal_variance_one",
        "model.icar_scaling",
    )
    exact(model["backend_capability"], "nuts", "model.capability")
    exact(model["maturity"], "experimental", "model.maturity")
    priors = {
        name: number(model[name], f"model.{name}")
        for name in [
            "intercept_prior_mean",
            "intercept_prior_sd",
            "coefficient_prior_sd",
            "total_sd_prior",
            "phi_alpha",
            "phi_beta",
        ]
    }
    if min(value for name, value in priors.items() if name != "intercept_prior_mean") <= 0.0:
        raise ContractError("BYM2 prior parameters must be positive")

    regions = areal.bym_regions(request, "scaled_icar_transform")
    original_variance = number(
        request["original_typical_marginal_variance"], "original typical variance"
    )
    scaled_variance = number(request["scaled_typical_marginal_variance"], "scaled typical variance")
    # Geometric mean of the scaled ICAR marginal variances (the diagonal of T T').
    marginal_variances = np.square(regions["transform"]).sum(axis=1)
    computed_scaled_variance = float(np.exp(np.log(marginal_variances).mean()))
    if original_variance <= 0.0 or abs(scaled_variance - 1.0) > 1e-12:
        raise ContractError("declared ICAR scaling is invalid")
    if abs(computed_scaled_variance - scaled_variance) > 1e-10:
        raise ContractError("scaled transform does not have unit typical marginal variance")
    sampling = nuts_sampling(request["sampling"], "sampling")
    chains, draws = sampling["chains"], sampling["draws"]
    output_limit = areal.dense_resources(
        request["resources"],
        "resources",
        len(regions["region_ids"]),
        chains * (sampling["tune"] + draws),
    )
    policy = nuts_policy(
        request["diagnostic_policy"],
        "policy",
        prior_draw_range=(100, 10_000),
        count_ceiling=chains * draws,
        check_thresholds=False,
    )
    return {**policy, **priors, **regions, **sampling, "output_limit": output_limit}


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-bym2-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the Poisson BYM2 model by NUTS.

    Likelihood: ``y_i ~ Poisson(E_i * exp(alpha + x_i' beta + b_i))`` with known expected
    counts ``E_i``. Priors: ``alpha ~ Normal(intercept_prior_mean, intercept_prior_sd)``,
    ``beta_j ~ Normal(0, coefficient_prior_sd)``, total SD ``sigma ~ HalfNormal(total_sd_prior)``,
    and mixing ``phi ~ Beta(phi_alpha, phi_beta)``. BYM2 parameterization (Riebler et al.):
    ``b = sigma * (sqrt(phi) * T z + sqrt(1 - phi) * v)`` with ``z, v ~ Normal(0, 1)``, where the
    request's ICAR basis ``T`` is scaled so the geometric mean of the ICAR marginal variances is
    one and ``T z`` sums to zero within every connected component.
    """
    dimension = len(config["region_ids"])
    predictors = len(config["predictor_names"])
    constrained_dimension = config["transform"].shape[1]
    coords = {"region": config["region_ids"], "predictor": config["predictor_names"]}
    with pm.Model(coords=coords):
        intercept = pm.Normal(
            "intercept", config["intercept_prior_mean"], config["intercept_prior_sd"]
        )
        coefficient = pm.Normal(
            "coefficient", 0.0, config["coefficient_prior_sd"], dims="predictor"
        )
        sigma = pm.HalfNormal("sigma", config["total_sd_prior"])
        phi = pm.Beta("phi", config["phi_alpha"], config["phi_beta"])
        structured_raw = pm.Normal("structured_raw", 0.0, 1.0, shape=constrained_dimension)
        unstructured_raw = pm.Normal("unstructured_raw", 0.0, 1.0, dims="region")
        structured_unit = pm.Deterministic(
            "structured_unit", pm.math.dot(config["transform"], structured_raw), dims="region"
        )
        structured_component = pm.Deterministic(
            "structured_component", sigma * pm.math.sqrt(phi) * structured_unit, dims="region"
        )
        unstructured_component = pm.Deterministic(
            "unstructured_component",
            sigma * pm.math.sqrt(1.0 - phi) * unstructured_raw,
            dims="region",
        )
        combined_effect = pm.Deterministic(
            "combined_effect", structured_component + unstructured_component, dims="region"
        )
        relative_risk = pm.Deterministic(
            "relative_risk",
            pm.math.exp(intercept + pm.math.dot(config["design"], coefficient) + combined_effect),
            dims="region",
        )
        pm.Poisson(
            "observed_count",
            mu=config["expected"] * relative_risk,
            observed=config["counts"],
            dims="region",
        )
        prior = pymc_nuts.prior_predictive(config, seed_for)
        posterior = pymc_nuts.nuts(config, seed_for)
        predictive = pymc_nuts.posterior_predictive(posterior, "observed_count", config, seed_for)

    monitored = ["intercept", "coefficient", "sigma", "phi", "structured_raw", "unstructured_raw"]
    fields = ["structured_component", "unstructured_component", "combined_effect", "relative_risk"]
    checks = areal.bym_posterior_checks(
        prior, posterior, predictive, monitored, ["structured_unit", *fields]
    )
    arrays = checks["arrays"]
    diagnostics = pymc_nuts_diagnostics(posterior, monitored)
    flat_structured_unit = arrays["structured_unit"].reshape(-1, dimension)
    constraints_valid = bool(
        np.all(arrays["sigma"] > 0.0)
        and np.all((arrays["phi"] > 0.0) & (arrays["phi"] < 1.0))
        and areal.component_sums_vanish(flat_structured_unit, config["components"])
    )
    complete = (
        checks["prior_finite"]
        and checks["posterior_finite"]
        and constraints_valid
        and convergence_passes(config, diagnostics)
    )
    flat_coefficient = arrays["coefficient"].reshape(-1, predictors)
    flat_fields = {name: arrays[name].reshape(-1, dimension) for name in fields}
    regions = [
        {
            "region_id": region_id,
            "observed_count": int(config["counts"][index]),
            "expected": float(config["expected"][index]),
            **{name: summary(flat_fields[name][:, index]) for name in fields},
        }
        for index, region_id in enumerate(config["region_ids"])
    ]
    return {
        "format": "marklab.pymc_bym2_worker_result",
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": sampling_record(config),
        "posterior": {
            "intercept": summary(arrays["intercept"]),
            "coefficients": [
                {"predictor": name, **summary(flat_coefficient[:, index])}
                for index, name in enumerate(config["predictor_names"])
            ],
            "sigma": summary(arrays["sigma"]),
            "phi": summary(arrays["phi"]),
        },
        "regions": regions,
        "diagnostics": {
            "prior_predictive_finite": checks["prior_finite"],
            "posterior_finite": checks["posterior_finite"],
            **diagnostics,
            "constraints_valid": constraints_valid,
            "identifiability_checks_passed": True,
        },
        "posterior_predictive": areal.count_predictive_check(
            config["counts"], checks["replicated"]
        ),
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(16 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=config["output_limit"])


if __name__ == "__main__":
    run_worker(main, "marklab PyMC worker failed")
