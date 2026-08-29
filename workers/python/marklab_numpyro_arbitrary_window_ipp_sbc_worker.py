#!/usr/bin/env python3
"""Prior-generative SBC for the weighted exact-window IPP likelihood."""

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


REQUEST_FORMAT = "marklab.numpyro_arbitrary_window_ipp_sbc_request"
RESULT_FORMAT = "marklab.arbitrary_window_ipp_sbc"
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


def integer(value: Any, path: str, low: int, high: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ContractError(f"{path} must be an integer in [{low},{high}]")
    return value


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
            "replicates",
            "maximum_generated_count",
            "maximum_total_work",
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
    config = load_pymc_contract().validate(
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
    digest = hashlib.sha256(
        f"marklab-numpyro-arbitrary-window-ipp-sbc-v1\0{seed}\0{purpose}\0{replicate}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


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
    intercept = numpyro.sample("intercept", dist.Normal(intercept_mean, intercept_sd))
    coefficient = numpyro.sample("coefficient", dist.Normal(coefficient_mean, coefficient_sd))
    expected = cell_area * jnp.exp(intercept + coefficient * covariate + offset)
    numpyro.sample("counts", dist.Poisson(expected).to_event(1), obs=counts)


def calibration(values: list[dict[str, Any]], name: str, draws: int) -> dict[str, Any]:
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
                    "true_total_expected_count": (
                        true_total if math.isfinite(true_total) else None
                    ),
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
        total = (
            config["cell_area"]
            * np.exp(
                intercept[:, None] + coefficient[:, None] * covariate + offset
            )
        ).sum(axis=1)
        posterior = az.from_dict(
            {"posterior": {
                "intercept": intercept.reshape(config["chains"], config["draws"]),
                "coefficient": coefficient.reshape(config["chains"], config["draws"]),
            }}
        )
        r_hat = float(
            max(
                np.asarray(az.rhat(posterior)["intercept"]).item(),
                np.asarray(az.rhat(posterior)["coefficient"]).item(),
            )
        )
        divergences = int(np.asarray(sampler.get_extra_fields()["diverging"]).sum())
        if not np.isfinite(intercept).all() or not np.isfinite(coefficient).all() or not np.isfinite(total).all():
            failures.append(
                {"replicate": replicate, "reason": "non_finite_posterior", "true_total_expected_count": true_total}
            )
            continue
        completed.append(
            {
                "replicate": replicate,
                "true_intercept": true_intercept,
                "true_coefficient": true_coefficient,
                "true_total_expected_count": true_total,
                "realized_total_count": int(counts.sum()),
                "intercept_rank": int(np.count_nonzero(intercept < true_intercept)),
                "coefficient_rank": int(np.count_nonzero(coefficient < true_coefficient)),
                "total_expected_count_rank": int(np.count_nonzero(total < true_total)),
                "intercept_covered_90": bool(np.quantile(intercept, 0.05) <= true_intercept <= np.quantile(intercept, 0.95)),
                "coefficient_covered_90": bool(np.quantile(coefficient, 0.05) <= true_coefficient <= np.quantile(coefficient, 0.95)),
                "total_expected_count_covered_90": bool(np.quantile(total, 0.05) <= true_total <= np.quantile(total, 0.95)),
                "r_hat": r_hat,
                "divergences": divergences,
            }
        )
    draws = config["chains"] * config["draws"]
    calibration_results = {
        name: calibration(completed, name, draws)
        for name in ("intercept", "coefficient", "total_expected_count")
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
    result = run_sbc(
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
            f"marklab NumPyro arbitrary-window IPP SBC failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
