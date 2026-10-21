#!/usr/bin/env python3
"""Bounded simulation-based calibration for Marklab's exact gridded LGCP."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import arviz as az
import jax

jax.config.update("jax_enable_x64", True)

import numpy as np
import numpyro
from numpyro.infer import MCMC, NUTS

sys.path.append(str(Path(__file__).resolve().parent))

import marklab_numpyro_gridded_lgcp_worker as lgcp
import marklab_pymc_gridded_lgcp_worker as pymc_contract
from marklab_worker_support import versions
from marklab_worker_support.diagnostics import (
    divergence_count,
    minimum_ebfmi,
    rank_convergence,
    tree_depth_hits,
)
from marklab_worker_support.numpyro_sbc import diagnostics_failure
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
    exact,
    file_sha256,
    integer,
    number,
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
from marklab_worker_support.sbc import calibration_passes, rank_and_coverage, rank_summary

REQUEST_FORMAT = "marklab.numpyro_gridded_lgcp_sbc_worker_request"
RESULT_FORMAT = "marklab.numpyro_gridded_lgcp_sbc_worker_result"
SBC_PARAMETERS = ("intercept", "coefficient", "latent_cell")
MONITORED = ["intercept", "coefficient", "field_raw"]


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format",
            "version",
            "backend",
            "jax_version",
            "source_request_sha256",
            "source_request",
            "calibration",
            "resources",
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
    pymc_worker_digest = file_sha256(pymc_contract.__file__)
    config = pymc_contract.validate(request["source_request"], lock_digest, pymc_worker_digest)
    calibration = obj(
        request["calibration"],
        {
            "replicates",
            "rank_bins",
            "interval_probability",
            "latent_cell_index",
            "minimum_rank_uniformity_p_value",
            "minimum_coverage",
            "maximum_coverage",
        },
        "calibration",
    )
    config.update(
        {
            "replicates": integer(calibration["replicates"], "calibration.replicates", 20, 100),
            "rank_bins": integer(calibration["rank_bins"], "calibration.rank_bins", 2, 100),
            "latent_cell_index": integer(
                calibration["latent_cell_index"],
                "calibration.latent_cell_index",
                0,
                config["covariate"].size - 1,
            ),
            "minimum_rank_p": number(
                calibration["minimum_rank_uniformity_p_value"], "calibration.minimum_rank_p"
            ),
            "minimum_coverage": number(
                calibration["minimum_coverage"], "calibration.minimum_coverage"
            ),
            "maximum_coverage": number(
                calibration["maximum_coverage"], "calibration.maximum_coverage"
            ),
        }
    )
    interval = number(calibration["interval_probability"], "calibration.interval")
    exact(interval, 0.9, "interval")
    resources = obj(
        request["resources"],
        {
            "maximum_replicates",
            "maximum_simulated_cells",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    replicates = config["replicates"]
    if replicates > integer(resources["maximum_replicates"], "resources.replicates", 1, 100):
        raise ContractError("replicates exceed resource limit")
    simulated_cells = replicates * config["covariate"].size
    if simulated_cells > integer(resources["maximum_simulated_cells"], "resources.cells", 1, 3600):
        raise ContractError("simulated cells exceed resource limit")
    iterations = replicates * config["chains"] * (config["tune"] + config["draws"])
    maximum_iterations = integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 1_000_000
    )
    if iterations > maximum_iterations:
        raise ContractError("iterations exceed resource limit")
    config["maximum_output_bytes"] = integer(
        resources["maximum_output_bytes"], "resources.output", 1, 2 * MIB
    )
    return config


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    return derive_seed("marklab-gridded-lgcp-sbc-v1", seed, purpose, replicate)


def run_sbc(config: dict[str, Any]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Simulate prior-generated count grids and refit the gridded LGCP to each.

    Each replicate draws the intercept, coefficient, and standard-normal field from their
    priors, forms ``u = L z``, simulates Poisson cell counts, and fits ``lgcp.model`` by NUTS.
    Replicates that fail the diagnostic policy or raise are recorded as failures.
    """
    kernel = NUTS(
        lgcp.model,
        target_accept_prob=config["target_accept"],
        max_tree_depth=config["maximum_tree_depth"],
    )
    completed: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    latent_index = config["latent_cell_index"]
    for replicate in range(config["replicates"]):
        try:
            rng = np.random.default_rng(seed_for(config["seed"], "simulate", replicate))
            true_intercept = float(rng.normal(config["intercept_mean"], config["intercept_sd"]))
            true_coefficient = float(
                rng.normal(config["coefficient_mean"], config["coefficient_sd"])
            )
            true_raw = rng.normal(0.0, 1.0, config["covariate"].size)
            true_latent = config["cholesky"] @ true_raw
            expected = config["cell_area"] * np.exp(
                true_intercept
                + true_coefficient * config["covariate"]
                + config["offset"]
                + true_latent
            )
            counts = rng.poisson(expected).astype(np.int64)
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
                **lgcp.model_arguments(config, counts),
                extra_fields=("diverging", "num_steps", "energy"),
            )
            samples = {
                name: np.asarray(value, dtype=np.float64)
                for name, value in sampler.get_samples(group_by_chain=True).items()
            }
            latent = np.einsum("ij,cdj->cdi", config["cholesky"], samples["field_raw"])
            posterior = az.from_dict({"posterior": {name: samples[name] for name in MONITORED}})
            r_hat, ess_bulk, ess_tail = rank_convergence(posterior, MONITORED)
            extra = sampler.get_extra_fields(group_by_chain=True)
            divergences = divergence_count(extra["diverging"])
            depth_hits = tree_depth_hits(extra["num_steps"], config["maximum_tree_depth"])
            ebfmi = minimum_ebfmi(np.asarray(extra["energy"], dtype=np.float64))
            failure = diagnostics_failure(
                {
                    "r_hat": r_hat,
                    "ess_bulk": ess_bulk,
                    "ess_tail": ess_tail,
                    "minimum_ebfmi": ebfmi,
                    "divergences": divergences,
                    "max_tree_depth_hits": depth_hits,
                },
                config,
            )
            if failure is not None:
                failures.append({"replicate": replicate, "reason": failure})
                continue
            truths = {
                "intercept": true_intercept,
                "coefficient": true_coefficient,
                "latent_cell": true_latent[latent_index],
            }
            draws = {
                "intercept": samples["intercept"].reshape(-1),
                "coefficient": samples["coefficient"].reshape(-1),
                "latent_cell": latent[..., latent_index].reshape(-1),
            }
            row: dict[str, Any] = {
                "replicate": replicate,
                "true_intercept": true_intercept,
                "true_coefficient": true_coefficient,
                "true_latent_cell": float(true_latent[latent_index]),
                "r_hat": r_hat,
                "ess_bulk": ess_bulk,
                "ess_tail": ess_tail,
                "minimum_ebfmi": ebfmi,
                "divergences": divergences,
                "max_tree_depth_hits": depth_hits,
            }
            for name in SBC_PARAMETERS:
                row[f"{name}_rank"], row[f"{name}_covered"] = rank_and_coverage(
                    draws[name], truths[name]
                )
            completed.append(row)
        except Exception as error:
            failures.append(
                {"replicate": replicate, "reason": f"{type(error).__name__}:{str(error)[:400]}"}
            )
    return completed, failures


def calibration_summary(
    config: dict[str, Any], completed: list[dict[str, Any]], failures: list[dict[str, Any]]
) -> tuple[dict[str, dict[str, Any]], bool]:
    """Summarize ranks and coverage per parameter and apply the calibration policy."""
    draws = config["chains"] * config["draws"]
    diagnostics = {
        name: rank_summary(
            [row[f"{name}_rank"] for row in completed],
            [row[f"{name}_covered"] for row in completed],
            draws,
            config["rank_bins"],
        )
        for name in SBC_PARAMETERS
    }
    passes = calibration_passes(
        diagnostics.values(),
        failures,
        config["minimum_rank_p"],
        config["minimum_coverage"],
        config["maximum_coverage"],
    )
    return diagnostics, passes


def main() -> int:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_digest, worker_digest = worker_digests(__file__)
    raw = read_request(2 * MIB)
    config = validate(json.loads(raw), lock_digest, worker_digest)
    completed, failures = run_sbc(config)
    diagnostics, passes = calibration_summary(config, completed, failures)
    result = {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_digest, worker_digest),
        "jax_version": jax.__version__,
        "request_sha256": sha256_hex(raw),
        "fit_state": "complete" if passes else "nonconverged",
        "replicates": completed,
        "failures": failures,
        "diagnostics": diagnostics,
    }
    write_result(result, max_bytes=config["maximum_output_bytes"], newline=False)
    return 0


if __name__ == "__main__":
    run_worker(main, "Marklab gridded LGCP SBC worker failed")
