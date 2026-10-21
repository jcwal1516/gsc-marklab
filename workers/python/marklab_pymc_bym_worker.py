#!/usr/bin/env python3
"""PyMC worker for Marklab's Poisson BYM disease-mapping model."""

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
    "icar_transform",
    "rank_deficiency",
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
    "structured_effect",
    "unstructured_effect",
    "intercept_prior_mean",
    "intercept_prior_sd",
    "coefficient_prior_sd",
    "structured_sd_prior",
    "unstructured_sd_prior",
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
    exact(model["family"], "poisson_bym_disease_mapping", "model.family")
    exact(model["likelihood"], "poisson_log_link", "model.likelihood")
    exact(model["offset"], "log_expected", "model.offset")
    exact(
        model["structured_effect"],
        "exact_sum_to_zero_noncentered_icar",
        "model.structured_effect",
    )
    exact(model["unstructured_effect"], "iid_standard_normal_scaled", "model.unstructured_effect")
    exact(model["backend_capability"], "nuts", "model.capability")
    exact(model["maturity"], "experimental", "model.maturity")
    priors = {
        name: number(model[name], f"model.{name}")
        for name in [
            "intercept_prior_mean",
            "intercept_prior_sd",
            "coefficient_prior_sd",
            "structured_sd_prior",
            "unstructured_sd_prior",
        ]
    }
    if min(value for name, value in priors.items() if name != "intercept_prior_mean") <= 0.0:
        raise ContractError("BYM prior scales must be positive")

    regions = areal.bym_regions(request, "icar_transform")
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
    return derive_seed("marklab-pymc-bym-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Fit the Poisson BYM model by NUTS.

    Likelihood: ``y_i ~ Poisson(E_i * exp(alpha + x_i' beta + s_i + u_i))`` with known expected
    counts ``E_i``. Priors: ``alpha ~ Normal(intercept_prior_mean, intercept_prior_sd)``,
    ``beta_j ~ Normal(0, coefficient_prior_sd)``, ``sigma_s ~ HalfNormal(structured_sd_prior)``,
    ``sigma_u ~ HalfNormal(unstructured_sd_prior)``. Non-centred effects: the structured ICAR
    effect is ``s = sigma_s * T z`` with ``z ~ Normal(0, 1)`` and ``T`` the request's ICAR basis,
    so ``s`` sums to zero within every connected component; ``u = sigma_u * v`` with
    ``v ~ Normal(0, 1)`` per region.
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
        structured_sd = pm.HalfNormal("structured_sd", config["structured_sd_prior"])
        unstructured_sd = pm.HalfNormal("unstructured_sd", config["unstructured_sd_prior"])
        structured_raw = pm.Normal("structured_raw", 0.0, 1.0, shape=constrained_dimension)
        unstructured_raw = pm.Normal("unstructured_raw", 0.0, 1.0, dims="region")
        structured = pm.Deterministic(
            "structured",
            structured_sd * pm.math.dot(config["transform"], structured_raw),
            dims="region",
        )
        unstructured = pm.Deterministic(
            "unstructured", unstructured_sd * unstructured_raw, dims="region"
        )
        log_relative_risk = pm.Deterministic(
            "log_relative_risk",
            intercept + pm.math.dot(config["design"], coefficient) + structured + unstructured,
            dims="region",
        )
        relative_risk = pm.Deterministic(
            "relative_risk", pm.math.exp(log_relative_risk), dims="region"
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

    monitored = [
        "intercept",
        "coefficient",
        "structured_sd",
        "unstructured_sd",
        "structured_raw",
        "unstructured_raw",
    ]
    checks = areal.bym_posterior_checks(
        prior,
        posterior,
        predictive,
        monitored,
        ["structured", "unstructured", "relative_risk"],
    )
    arrays = checks["arrays"]
    diagnostics = pymc_nuts_diagnostics(posterior, monitored)
    flat_structured = arrays["structured"].reshape(-1, dimension)
    constraints_valid = bool(
        np.all(arrays["structured_sd"] > 0.0)
        and np.all(arrays["unstructured_sd"] > 0.0)
        and areal.component_sums_vanish(flat_structured, config["components"])
    )
    complete = (
        checks["prior_finite"]
        and checks["posterior_finite"]
        and constraints_valid
        and convergence_passes(config, diagnostics)
    )
    flat_coefficient = arrays["coefficient"].reshape(-1, predictors)
    flat_unstructured = arrays["unstructured"].reshape(-1, dimension)
    flat_risk = arrays["relative_risk"].reshape(-1, dimension)
    regions = [
        {
            "region_id": region_id,
            "observed_count": int(config["counts"][index]),
            "expected": float(config["expected"][index]),
            "structured_effect": summary(flat_structured[:, index]),
            "unstructured_effect": summary(flat_unstructured[:, index]),
            "relative_risk": summary(flat_risk[:, index]),
        }
        for index, region_id in enumerate(config["region_ids"])
    ]
    return {
        "format": "marklab.pymc_bym_worker_result",
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
            "structured_sd": summary(arrays["structured_sd"]),
            "unstructured_sd": summary(arrays["unstructured_sd"]),
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
