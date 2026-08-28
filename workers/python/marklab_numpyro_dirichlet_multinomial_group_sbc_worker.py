#!/usr/bin/env python3
"""Bounded SBC for patient Dirichlet-multinomial group regression."""

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


REQUEST_FORMAT = "marklab.numpyro_dirichlet_multinomial_group_sbc_worker_request"
RESULT_FORMAT = "marklab.numpyro_dirichlet_multinomial_group_sbc_worker_result"


class ContractError(ValueError):
    pass


def load_module(name: str) -> Any:
    path = Path(__file__).with_name(name)
    specification = importlib.util.spec_from_file_location(name.removesuffix(".py"), path)
    if specification is None or specification.loader is None:
        raise ContractError(f"cannot load {name}")
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


def validate(request: Any, lock_sha: str, worker_sha: str) -> dict[str, Any]:
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
        "marklab_pymc_dirichlet_multinomial_group_worker.py"
    )
    pymc = load_module(pymc_path.name)
    config = pymc.validate(
        request["source_request"],
        lock_sha,
        hashlib.sha256(pymc_path.read_bytes()).hexdigest(),
    )
    calibration = obj(
        request["calibration"],
        {
            "replicates", "rank_bins", "interval_probability", "maximum_tree_depth",
            "minimum_rank_uniformity_p_value", "minimum_coverage", "maximum_coverage",
        },
        "calibration",
    )
    config.update(
        {
            "replicates": integer(calibration["replicates"], "calibration.replicates", 20, 100),
            "rank_bins": integer(calibration["rank_bins"], "calibration.rank_bins", 2, 100),
            "maximum_tree_depth": integer(
                calibration["maximum_tree_depth"], "calibration.maximum_tree_depth", 10, 16
            ),
            "minimum_rank_p": number(
                calibration["minimum_rank_uniformity_p_value"], "calibration.rank_p"
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
    total_cells = int(np.asarray(config["totals"]).sum())
    if (
        config["replicates"]
        > integer(resources["maximum_replicates"], "resources.replicates", 1, 100)
        or config["replicates"] * total_cells
        > integer(resources["maximum_simulated_cells"], "resources.cells", 1, 200_000_000)
        or config["replicates"] * config["chains"] * (config["tune"] + config["draws"])
        > integer(resources["maximum_total_iterations"], "resources.iterations", 1, 1_000_000)
    ):
        raise ContractError("SBC resources exceeded")
    config["maximum_output_bytes"] = integer(
        resources["maximum_output_bytes"], "resources.output", 1, 2 * 1_048_576
    )
    return config


def seed_for(seed: int, purpose: str, replicate: int) -> int:
    digest = hashlib.sha256(
        f"marklab-dirichlet-multinomial-group-sbc-v1\0{seed}\0{purpose}\0{replicate}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def softmax(values: np.ndarray) -> np.ndarray:
    shifted = values - values.max(axis=-1, keepdims=True)
    weights = np.exp(shifted)
    return weights / weights.sum(axis=-1, keepdims=True)


def ranks(draws: np.ndarray, truth: np.ndarray) -> list[int]:
    return [int(np.sum(draws[:, index] < truth[index])) for index in range(draws.shape[1])]


def covered(draws: np.ndarray, truth: np.ndarray) -> list[bool]:
    return [
        bool(np.quantile(draws[:, index], 0.05) <= truth[index] <= np.quantile(draws[:, index], 0.95))
        for index in range(draws.shape[1])
    ]


def run_sbc(config: dict[str, Any]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    agreement = load_module("marklab_numpyro_dirichlet_multinomial_group_worker.py")
    totals = np.asarray(config["totals"], dtype=np.int64)
    indicator = np.asarray(config["indicator"], dtype=np.int64)
    classes = len(config["class_ids"])
    total_count_max = int(totals.max())
    kernel = NUTS(
        agreement.model,
        target_accept_prob=config["target_accept"],
        max_tree_depth=config["maximum_tree_depth"],
        dense_mass=True,
    )
    completed: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    for replicate in range(config["replicates"]):
        try:
            rng = np.random.default_rng(seed_for(config["seed"], "simulate", replicate))
            true_baseline = rng.normal(0.0, config["logit_sd"], classes - 1)
            true_effect = rng.normal(0.0, config["group_sd"], classes - 1)
            true_concentration = float(abs(rng.normal(0.0, config["concentration_sd"])))
            true_reference = softmax(np.concatenate((true_baseline, [0.0]))[None, :])[0]
            true_comparison = softmax(
                np.concatenate((true_baseline + true_effect, [0.0]))[None, :]
            )[0]
            true_difference = true_comparison - true_reference
            group_probabilities = np.stack((true_reference, true_comparison))[indicator]
            patient_probabilities = np.asarray(
                [rng.dirichlet(probability * true_concentration) for probability in group_probabilities]
            )
            observations = np.asarray(
                [rng.multinomial(int(total), probability) for total, probability in zip(totals, patient_probabilities)],
                dtype=np.int64,
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
                observed_counts=jnp.asarray(observations),
                totals=jnp.asarray(totals),
                total_count_max=total_count_max,
                indicator=jnp.asarray(indicator),
                classes=classes,
                logit_sd=config["logit_sd"],
                group_sd=config["group_sd"],
                concentration_sd=config["concentration_sd"],
                extra_fields=("diverging", "num_steps", "energy"),
            )
            samples = {
                name: np.asarray(value, dtype=np.float64)
                for name, value in sampler.get_samples(group_by_chain=True).items()
            }
            monitored = {
                "baseline_logits": samples["baseline_logits"],
                "group_log_ratio_effects": samples["group_log_ratio_effects"],
                "concentration": samples["concentration"],
            }
            inference = az.from_dict({"posterior": monitored})
            names = list(monitored)
            r_hat = float(flattened(az.rhat(inference, var_names=names, method="rank"), names).max())
            bulk = float(flattened(az.ess(inference, var_names=names, method="bulk"), names).min())
            tail = float(flattened(az.ess(inference, var_names=names, method="tail"), names).min())
            extra = sampler.get_extra_fields(group_by_chain=True)
            energy = np.asarray(extra["energy"], dtype=np.float64)
            ebfmi = float(
                np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1))
            )
            divergences = int(np.asarray(extra["diverging"]).sum())
            depth_hits = int(
                (np.asarray(extra["num_steps"]) >= 2 ** config["maximum_tree_depth"] - 1).sum()
            )
            if not (
                r_hat <= config["maximum_r_hat"]
                and bulk >= config["minimum_bulk_ess"]
                and tail >= config["minimum_tail_ess"]
                and ebfmi >= config["minimum_ebfmi"]
                and divergences <= config["maximum_divergences"]
                and depth_hits <= config["maximum_tree_depth_hits"]
            ):
                failures.append(
                    {
                        "replicate": replicate,
                        "reason": (
                            f"diagnostics_failed:r_hat={r_hat:.17g},bulk={bulk:.17g},"
                            f"tail={tail:.17g},ebfmi={ebfmi:.17g},"
                            f"divergences={divergences},depth_hits={depth_hits}"
                        ),
                    }
                )
                continue
            baseline_draws = samples["baseline_logits"].reshape(-1, classes - 1)
            effect_draws = samples["group_log_ratio_effects"].reshape(-1, classes - 1)
            concentration_draws = samples["concentration"].reshape(-1)
            zeros = np.zeros((baseline_draws.shape[0], 1), dtype=np.float64)
            reference_draws = softmax(np.concatenate((baseline_draws, zeros), axis=1))
            comparison_draws = softmax(np.concatenate((baseline_draws + effect_draws, zeros), axis=1))
            difference_draws = comparison_draws - reference_draws
            row = {
                "replicate": replicate,
                "true_baseline_logits": true_baseline.tolist(),
                "true_group_log_ratio_effects": true_effect.tolist(),
                "true_concentration": true_concentration,
                "true_class_probability_differences": true_difference.tolist(),
                "baseline_logit_ranks": ranks(baseline_draws, true_baseline),
                "group_log_ratio_effect_ranks": ranks(effect_draws, true_effect),
                "concentration_rank": int(np.sum(concentration_draws < true_concentration)),
                "class_probability_difference_ranks": ranks(difference_draws, true_difference),
                "baseline_logit_covered": covered(baseline_draws, true_baseline),
                "group_log_ratio_effect_covered": covered(effect_draws, true_effect),
                "concentration_covered": bool(
                    np.quantile(concentration_draws, 0.05)
                    <= true_concentration
                    <= np.quantile(concentration_draws, 0.95)
                ),
                "class_probability_difference_covered": covered(
                    difference_draws, true_difference
                ),
                "r_hat": r_hat,
                "ess_bulk": bulk,
                "ess_tail": tail,
                "minimum_ebfmi": ebfmi,
                "divergences": divergences,
                "max_tree_depth_hits": depth_hits,
            }
            completed.append(row)
        except Exception as error:
            failures.append(
                {"replicate": replicate, "reason": f"{type(error).__name__}:{str(error)[:400]}"}
            )
    return completed, failures


def aggregate_values(ranks: list[int], coverage: list[bool], draws: int, bins: int) -> dict[str, Any]:
    rank_array = np.asarray(ranks, dtype=np.int64)
    histogram = np.zeros(bins, dtype=np.int64)
    for rank in rank_array:
        histogram[min(int(rank) * bins // (draws + 1), bins - 1)] += 1
    return {
        "rank_histogram": histogram.tolist(),
        "rank_uniformity_p_value": float(stats.chisquare(histogram).pvalue) if ranks else 0.0,
        "coverage_90": float(np.mean(coverage)) if coverage else 0.0,
        "mean_normalized_rank": float(rank_array.mean() / draws) if ranks else 0.0,
    }


def aggregate_vector(
    rows: list[dict[str, Any]], rank_name: str, coverage_name: str, width: int, draws: int, bins: int
) -> list[dict[str, Any]]:
    return [
        aggregate_values(
            [row[rank_name][index] for row in rows],
            [row[coverage_name][index] for row in rows],
            draws,
            bins,
        )
        for index in range(width)
    ]


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
    config = validate(json.loads(raw), lock_sha, worker_sha)
    completed, failures = run_sbc(config)
    total_draws = config["chains"] * config["draws"]
    free = len(config["class_ids"]) - 1
    diagnostics = {
        "baseline_logits": aggregate_vector(
            completed, "baseline_logit_ranks", "baseline_logit_covered", free,
            total_draws, config["rank_bins"],
        ),
        "group_log_ratio_effects": aggregate_vector(
            completed, "group_log_ratio_effect_ranks", "group_log_ratio_effect_covered", free,
            total_draws, config["rank_bins"],
        ),
        "concentration": aggregate_values(
            [row["concentration_rank"] for row in completed],
            [row["concentration_covered"] for row in completed],
            total_draws,
            config["rank_bins"],
        ),
        "class_probability_differences": aggregate_vector(
            completed,
            "class_probability_difference_ranks",
            "class_probability_difference_covered",
            len(config["class_ids"]),
            total_draws,
            config["rank_bins"],
        ),
    }
    all_diagnostics = (
        diagnostics["baseline_logits"]
        + diagnostics["group_log_ratio_effects"]
        + [diagnostics["concentration"]]
        + diagnostics["class_probability_differences"]
    )
    passes = not failures and all(
        config["minimum_rank_p"] <= value["rank_uniformity_p_value"]
        and config["minimum_coverage"] <= value["coverage_90"] <= config["maximum_coverage"]
        for value in all_diagnostics
    )
    result = {
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
        print(
            f"Marklab Dirichlet-multinomial group SBC worker failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
