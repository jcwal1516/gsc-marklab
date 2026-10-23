#!/usr/bin/env python3
"""Prior-generative SBC for the weighted exact-window IPP likelihood."""

from __future__ import annotations

import json
import math
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

from marklab_worker_support import inhomogeneous_poisson as source_contract
from marklab_worker_support import versions
from marklab_worker_support.diagnostics import divergence_count
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
    exact,
    file_sha256,
    integer,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)
from marklab_worker_support.sbc import rank_and_coverage

REQUEST_FORMAT = "marklab.numpyro_arbitrary_window_ipp_sbc_request"
RESULT_FORMAT = "marklab.arbitrary_window_ipp_sbc"
SBC_PARAMETERS = ("intercept", "coefficient", "total_expected_count")


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
            "replicates",
            "maximum_generated_count",
            "maximum_total_work",
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
    config = source_contract.validate_request(
        request["source_request"], lock_digest, pymc_worker_digest
    )
    replicates = integer(request["replicates"], "replicates", 20, 100)
    maximum_generated_count = integer(
        request["maximum_generated_count"], "maximum_generated_count", 1, 10_000_000
    )
    maximum_total_work = integer(
        request["maximum_total_work"], "maximum_total_work", 1, 100_000_000
    )
    total_work = (
        replicates
        * config["chains"]
        * (config["tune"] + config["draws"])
        * config["grid_covariate"].size
    )
    if total_work > maximum_total_work:
        raise ContractError("SBC total work exceeds the declared maximum")
    config.update(
        replicates=replicates,
        maximum_generated_count=maximum_generated_count,
        maximum_total_work=maximum_total_work,
        total_work=total_work,
    )
    return request, config


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    return derive_seed("marklab-numpyro-arbitrary-window-ipp-sbc-v1", seed, purpose, replicate)


def model(
    covariate: jnp.ndarray,
    offset: jnp.ndarray,
    cell_area: float,
    counts: jnp.ndarray,
    intercept_mean: float,
    intercept_sd: float,
    coefficient_mean: float,
    coefficient_sd: float,
) -> None:
    """Gridded Poisson approximation of the log-linear IPP used to refit SBC replicates.

    Likelihood: ``n_k ~ Poisson(A * exp(alpha + beta * covariate_k + offset_k))`` for each
    quadrature node of area ``A``. Priors: ``alpha ~ Normal(intercept_mean, intercept_sd)`` and
    ``beta ~ Normal(coefficient_mean, coefficient_sd)``.
    """
    intercept = numpyro.sample("intercept", dist.Normal(intercept_mean, intercept_sd))
    coefficient = numpyro.sample("coefficient", dist.Normal(coefficient_mean, coefficient_sd))
    expected = cell_area * jnp.exp(intercept + coefficient * covariate + offset)
    numpyro.sample("counts", dist.Poisson(expected).to_event(1), obs=counts)


def calibration(values: list[dict[str, Any]], name: str, draws: int) -> dict[str, Any]:
    """Accept a parameter when its mean normalized rank is in [0.25, 0.75] and its 90%
    coverage is in [0.65, 1]."""
    if not values:
        return {
            "replicates": 0,
            "posterior_draws_per_replicate": draws,
            "normalized_mean_rank": 0.0,
            "coverage_90": 0.0,
            "accepted": False,
        }
    ranks = np.asarray([value[f"{name}_rank"] for value in values], dtype=np.float64)
    coverage = float(np.mean([value[f"{name}_covered_90"] for value in values]))
    normalized_mean_rank = float(np.mean(ranks / draws))
    accepted = 0.25 <= normalized_mean_rank <= 0.75 and 0.65 <= coverage <= 1.0
    return {
        "replicates": len(values),
        "posterior_draws_per_replicate": draws,
        "normalized_mean_rank": normalized_mean_rank,
        "coverage_90": coverage,
        "accepted": accepted,
    }


def run_sbc(
    config: dict[str, Any],
    request_sha256: str,
    source_request_sha256: str,
    lock_digest: str,
    worker_digest: str,
) -> dict[str, Any]:
    """Draw intercept and coefficient from their priors, simulate node counts, and refit."""
    completed: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    covariate = np.asarray(config["grid_covariate"], dtype=np.float64)
    offset = np.asarray(config["grid_offset"], dtype=np.float64)
    for replicate in range(config["replicates"]):
        rng = np.random.default_rng(seed_for(config["seed"], "generate", replicate))
        true_intercept = float(
            rng.normal(config["intercept_prior_mean"], config["intercept_prior_sd"])
        )
        true_coefficient = float(
            rng.normal(config["coefficient_prior_mean"], config["coefficient_prior_sd"])
        )
        true_expected = config["cell_area"] * np.exp(
            true_intercept + true_coefficient * covariate + offset
        )
        true_total = float(true_expected.sum())
        if not np.isfinite(true_expected).all() or true_total > config["maximum_generated_count"]:
            failures.append(
                {
                    "replicate": replicate,
                    "reason": "generated_count_resource_ceiling",
                    "true_total_expected_count": true_total if math.isfinite(true_total) else None,
                }
            )
            continue
        counts = rng.poisson(true_expected).astype(np.int64)
        if int(counts.sum()) > config["maximum_generated_count"]:
            failures.append(
                {
                    "replicate": replicate,
                    "reason": "realized_count_resource_ceiling",
                    "true_total_expected_count": true_total,
                }
            )
            continue
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
            jax.random.PRNGKey(seed_for(config["seed"], "fit", replicate)),
            covariate=jnp.asarray(covariate),
            offset=jnp.asarray(offset),
            cell_area=config["cell_area"],
            counts=jnp.asarray(counts),
            intercept_mean=config["intercept_prior_mean"],
            intercept_sd=config["intercept_prior_sd"],
            coefficient_mean=config["coefficient_prior_mean"],
            coefficient_sd=config["coefficient_prior_sd"],
            extra_fields=("diverging",),
        )
        samples = {
            name: np.asarray(value, dtype=np.float64).reshape(-1)
            for name, value in sampler.get_samples(group_by_chain=True).items()
        }
        intercept = samples["intercept"]
        coefficient = samples["coefficient"]
        node_expected = config["cell_area"] * np.exp(
            intercept[:, None] + coefficient[:, None] * covariate + offset
        )
        total = node_expected.sum(axis=1)
        chain_shape = (config["chains"], config["draws"])
        posterior = az.from_dict(
            {
                "posterior": {
                    "intercept": intercept.reshape(chain_shape),
                    "coefficient": coefficient.reshape(chain_shape),
                }
            }
        )
        r_hat_values = az.rhat(posterior)
        r_hat = float(
            max(
                np.asarray(r_hat_values["intercept"]).item(),
                np.asarray(r_hat_values["coefficient"]).item(),
            )
        )
        divergences = divergence_count(sampler.get_extra_fields()["diverging"])
        if (
            not np.isfinite(intercept).all()
            or not np.isfinite(coefficient).all()
            or not np.isfinite(total).all()
        ):
            failures.append(
                {
                    "replicate": replicate,
                    "reason": "non_finite_posterior",
                    "true_total_expected_count": true_total,
                }
            )
            continue
        truths = {
            "intercept": true_intercept,
            "coefficient": true_coefficient,
            "total_expected_count": true_total,
        }
        draws = {"intercept": intercept, "coefficient": coefficient, "total_expected_count": total}
        row: dict[str, Any] = {
            "replicate": replicate,
            "true_intercept": true_intercept,
            "true_coefficient": true_coefficient,
            "true_total_expected_count": true_total,
            "realized_total_count": int(counts.sum()),
            "r_hat": r_hat,
            "divergences": divergences,
        }
        for name in SBC_PARAMETERS:
            row[f"{name}_rank"], row[f"{name}_covered_90"] = rank_and_coverage(
                draws[name], truths[name]
            )
        completed.append(row)
    posterior_draws = config["chains"] * config["draws"]
    calibration_results = {
        name: calibration(completed, name, posterior_draws) for name in SBC_PARAMETERS
    }
    accepted = (
        not failures
        and len(completed) == config["replicates"]
        and all(value["accepted"] for value in calibration_results.values())
        and all(value["divergences"] == 0 and value["r_hat"] <= 1.05 for value in completed)
    )
    return {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_digest, worker_digest),
        "jax_version": jax.__version__,
        "request_sha256": request_sha256,
        "source_request_sha256": source_request_sha256,
        "requested_replicates": config["replicates"],
        "completed_replicates": len(completed),
        "failed_replicates": len(failures),
        "replicates": completed,
        "failures": failures,
        "calibration": calibration_results,
        "calibration_status": "accepted" if accepted else "not_accepted",
        "total_work": config["total_work"],
        "maximum_generated_count": config["maximum_generated_count"],
        "claim_status": "synthetic_prior_generative_calibration_only",
    }


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    pymc_worker_digest = file_sha256(Path(__file__).with_name(source_contract.SOURCE_WORKER))
    raw = read_request(16 * MIB)
    request, config = validate(json.loads(raw), lock_digest, worker_digest, pymc_worker_digest)
    result = run_sbc(
        config,
        sha256_hex(raw),
        request["source_request_sha256"],
        lock_digest,
        worker_digest,
    )
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "marklab NumPyro arbitrary-window IPP SBC failed")
