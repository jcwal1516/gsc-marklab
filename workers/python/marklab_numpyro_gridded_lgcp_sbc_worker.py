#!/usr/bin/env python3
"""Bounded simulation-based calibration for Marklab's exact gridded LGCP."""

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
from numpyro.infer import MCMC, NUTS
from scipy import stats


REQUEST_FORMAT = "marklab.numpyro_gridded_lgcp_sbc_worker_request"
RESULT_FORMAT = "marklab.numpyro_gridded_lgcp_sbc_worker_result"


class ContractError(ValueError):
    pass


def load_lgcp_worker() -> Any:
    path = Path(__file__).with_name("marklab_numpyro_gridded_lgcp_worker.py")
    specification = importlib.util.spec_from_file_location("marklab_numpyro_lgcp", path)
    if specification is None or specification.loader is None:
        raise ContractError("cannot load the NumPyro gridded LGCP model")
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


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {
            "format", "version", "backend", "jax_version", "source_request_sha256",
            "source_request", "calibration", "resources",
        },
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "request.backend",
    )
    exact(backend["name"], "numpyro", "backend.name")
    exact(backend["version"], "0.21.0", "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    exact(request["jax_version"], "0.11.1", "request.jax_version")
    source_digest = request["source_request_sha256"]
    if (
        not isinstance(source_digest, str)
        or len(source_digest) != 64
        or any(character not in "0123456789abcdef" for character in source_digest)
    ):
        raise ContractError("source request digest is invalid")
    script = Path(__file__)
    pymc_worker_digest = hashlib.sha256(
        script.with_name("marklab_pymc_gridded_lgcp_worker.py").read_bytes()
    ).hexdigest()
    lgcp = load_lgcp_worker()
    config = lgcp.load_pymc_contract().validate(
        request["source_request"], lock_digest, pymc_worker_digest
    )
    calibration = obj(
        request["calibration"],
        {
            "replicates", "rank_bins", "interval_probability", "latent_cell_index",
            "minimum_rank_uniformity_p_value", "minimum_coverage", "maximum_coverage",
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
    exact(number(calibration["interval_probability"], "calibration.interval"), 0.9, "interval")
    resources = obj(
        request["resources"],
        {
            "maximum_replicates", "maximum_simulated_cells", "maximum_total_iterations",
            "maximum_output_bytes", "timeout_seconds",
        },
        "resources",
    )
    if config["replicates"] > integer(
        resources["maximum_replicates"], "resources.replicates", 1, 100
    ):
        raise ContractError("replicates exceed resource limit")
    if config["replicates"] * config["covariate"].size > integer(
        resources["maximum_simulated_cells"], "resources.cells", 1, 3600
    ):
        raise ContractError("simulated cells exceed resource limit")
    if config["replicates"] * config["chains"] * (config["tune"] + config["draws"]) > integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 1_000_000
    ):
        raise ContractError("iterations exceed resource limit")
    config["maximum_output_bytes"] = integer(
        resources["maximum_output_bytes"], "resources.output", 1, 2 * 1_048_576
    )
    return config


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    digest = hashlib.sha256(
        f"marklab-gridded-lgcp-sbc-v1\0{seed}\0{purpose}\0{replicate}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def run_sbc(config: dict[str, Any]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    lgcp = load_lgcp_worker()
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
                covariate=jnp.asarray(config["covariate"]),
                offset=jnp.asarray(config["offset"]),
                counts=jnp.asarray(counts),
                cholesky=jnp.asarray(config["cholesky"]),
                cell_area=config["cell_area"],
                intercept_mean=config["intercept_mean"],
                intercept_sd=config["intercept_sd"],
                coefficient_mean=config["coefficient_mean"],
                coefficient_sd=config["coefficient_sd"],
                extra_fields=("diverging", "num_steps", "energy"),
            )
            samples = {
                name: np.asarray(value, dtype=np.float64)
                for name, value in sampler.get_samples(group_by_chain=True).items()
            }
            latent = np.einsum("ij,cdj->cdi", config["cholesky"], samples["field_raw"])
            posterior = az.from_dict(
                {
                    "posterior": {
                        "intercept": samples["intercept"],
                        "coefficient": samples["coefficient"],
                        "field_raw": samples["field_raw"],
                    }
                }
            )
            names = ["intercept", "coefficient", "field_raw"]
            r_hat = float(flattened(az.rhat(posterior, var_names=names, method="rank"), names).max())
            ess_bulk = float(flattened(az.ess(posterior, var_names=names, method="bulk"), names).min())
            ess_tail = float(flattened(az.ess(posterior, var_names=names, method="tail"), names).min())
            extra = sampler.get_extra_fields(group_by_chain=True)
            divergences = int(np.asarray(extra["diverging"]).sum())
            depth_hits = int(
                (np.asarray(extra["num_steps"]) >= 2 ** config["maximum_tree_depth"] - 1).sum()
            )
            energy = np.asarray(extra["energy"], dtype=np.float64)
            minimum_ebfmi = float(
                np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1))
            )
            diagnostics_pass = (
                r_hat <= config["maximum_r_hat"]
                and ess_bulk >= config["minimum_bulk_ess"]
                and ess_tail >= config["minimum_tail_ess"]
                and minimum_ebfmi >= config["minimum_ebfmi"]
                and divergences <= config["maximum_divergences"]
                and depth_hits <= config["maximum_tree_depth_hits"]
            )
            if not diagnostics_pass:
                failures.append(
                    {
                        "replicate": replicate,
                        "reason": (
                            f"diagnostics_failed:r_hat={r_hat:.17g},bulk={ess_bulk:.17g},"
                            f"tail={ess_tail:.17g},ebfmi={minimum_ebfmi:.17g},"
                            f"divergences={divergences},depth_hits={depth_hits}"
                        ),
                    }
                )
                continue
            intercept_draws = samples["intercept"].reshape(-1)
            coefficient_draws = samples["coefficient"].reshape(-1)
            latent_draws = latent[..., latent_index].reshape(-1)
            completed.append(
                {
                    "replicate": replicate,
                    "true_intercept": true_intercept,
                    "true_coefficient": true_coefficient,
                    "true_latent_cell": float(true_latent[latent_index]),
                    "intercept_rank": int(np.sum(intercept_draws < true_intercept)),
                    "coefficient_rank": int(np.sum(coefficient_draws < true_coefficient)),
                    "latent_cell_rank": int(np.sum(latent_draws < true_latent[latent_index])),
                    "intercept_covered": bool(
                        np.quantile(intercept_draws, 0.05)
                        <= true_intercept
                        <= np.quantile(intercept_draws, 0.95)
                    ),
                    "coefficient_covered": bool(
                        np.quantile(coefficient_draws, 0.05)
                        <= true_coefficient
                        <= np.quantile(coefficient_draws, 0.95)
                    ),
                    "latent_cell_covered": bool(
                        np.quantile(latent_draws, 0.05)
                        <= true_latent[latent_index]
                        <= np.quantile(latent_draws, 0.95)
                    ),
                    "r_hat": r_hat,
                    "ess_bulk": ess_bulk,
                    "ess_tail": ess_tail,
                    "minimum_ebfmi": minimum_ebfmi,
                    "divergences": divergences,
                    "max_tree_depth_hits": depth_hits,
                }
            )
        except Exception as error:
            failures.append(
                {"replicate": replicate, "reason": f"{type(error).__name__}:{str(error)[:400]}"}
            )
    return completed, failures


def aggregate(
    completed: list[dict[str, Any]], rank: str, covered: str, draws: int, bins: int
) -> dict[str, Any]:
    ranks = np.asarray([row[rank] for row in completed], dtype=np.int64)
    histogram = np.zeros(bins, dtype=np.int64)
    for value in ranks:
        histogram[min(int(value) * bins // (draws + 1), bins - 1)] += 1
    return {
        "rank_histogram": histogram.tolist(),
        "rank_uniformity_p_value": float(stats.chisquare(histogram).pvalue)
        if len(completed)
        else 0.0,
        "coverage_90": float(np.mean([row[covered] for row in completed]))
        if completed
        else 0.0,
        "mean_normalized_rank": float(ranks.mean() / draws) if len(completed) else 0.0,
    }


def main() -> int:
    script = Path(__file__).resolve()
    lock_digest = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_digest = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(2 * 1024 * 1024 + 1)
    if not raw or len(raw) > 2 * 1024 * 1024:
        raise ContractError("request size is invalid")
    config = validate(json.loads(raw), lock_digest, worker_digest)
    completed, failures = run_sbc(config)
    draws = config["chains"] * config["draws"]
    diagnostics = {
        "intercept": aggregate(completed, "intercept_rank", "intercept_covered", draws, config["rank_bins"]),
        "coefficient": aggregate(
            completed, "coefficient_rank", "coefficient_covered", draws, config["rank_bins"]
        ),
        "latent_cell": aggregate(
            completed, "latent_cell_rank", "latent_cell_covered", draws, config["rank_bins"]
        ),
    }
    passes = not failures and all(
        config["minimum_rank_p"] <= value["rank_uniformity_p_value"]
        and config["minimum_coverage"] <= value["coverage_90"] <= config["maximum_coverage"]
        for value in diagnostics.values()
    )
    result = {
        "format": RESULT_FORMAT,
        "version": 1,
        "backend": {
            "name": "numpyro", "version": numpyro.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_digest, "worker_sha256": worker_digest,
        },
        "jax_version": jax.__version__,
        "request_sha256": hashlib.sha256(raw).hexdigest(),
        "fit_state": "complete" if passes else "nonconverged",
        "replicates": completed,
        "failures": failures,
        "diagnostics": diagnostics,
    }
    encoded = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")).encode()
    if len(encoded) > config["maximum_output_bytes"]:
        raise ContractError("result exceeds output limit")
    sys.stdout.buffer.write(encoded)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"Marklab gridded LGCP SBC worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
