#!/usr/bin/env python3
"""NumPyro coregionalized replicated multitype LGCP on exact quadrature windows."""

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


REQUEST_FORMAT = (
    "marklab.numpyro_correlated_replicated_arbitrary_window_multitype_lgcp_request"
)
RESULT_FORMAT = (
    "marklab.numpyro_correlated_replicated_arbitrary_window_multitype_lgcp_result"
)
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"


class ContractError(ValueError):
    pass


def load(filename: str, name: str) -> Any:
    path = Path(__file__).with_name(filename)
    specification = importlib.util.spec_from_file_location(name, path)
    if specification is None or specification.loader is None:
        raise ContractError(f"cannot load {filename}")
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


def validate(
    request: Any,
    lock_sha: str,
    worker_sha: str,
    pymc_inferred_sha: str,
    pymc_fixed_sha: str,
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
            "correlation_prior",
            "resources",
            "maximum_tree_depth",
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
    exact(backend["version"], NUMPYRO_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python_version")
    exact(backend["environment_lock_sha256"], lock_sha, "backend.lock")
    exact(backend["worker_sha256"], worker_sha, "backend.worker")
    exact(request["jax_version"], JAX_VERSION, "request.jax_version")
    source_sha = request["source_request_sha256"]
    if not isinstance(source_sha, str) or len(source_sha) != 64:
        raise ContractError("source request digest is invalid")
    pymc = load(
        "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py",
        "marklab_pymc_inferred_multitype_contract_for_correlation",
    )
    _, config = pymc.validate(
        request["source_request"], lock_sha, pymc_inferred_sha, pymc_fixed_sha
    )
    prior = obj(
        request["correlation_prior"],
        {"lkj_concentration", "marginal_field_scale"},
        "correlation_prior",
    )
    if (
        not isinstance(prior["lkj_concentration"], (int, float))
        or not math.isfinite(prior["lkj_concentration"])
        or prior["lkj_concentration"] < 1.0
        or not isinstance(prior["marginal_field_scale"], (int, float))
        or not math.isfinite(prior["marginal_field_scale"])
        or prior["marginal_field_scale"] <= 0.0
    ):
        raise ContractError("correlation prior is invalid")
    resources = obj(
        request["resources"],
        {
            "coregionalization_work",
            "maximum_coregionalization_work",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    for name in resources:
        value = resources[name]
        if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
            raise ContractError(f"resources.{name} must be a positive integer")
    if resources["coregionalization_work"] > resources["maximum_coregionalization_work"]:
        raise ContractError("coregionalization work exceeds its maximum")
    depth = request["maximum_tree_depth"]
    if isinstance(depth, bool) or not isinstance(depth, int) or not 10 <= depth <= 14:
        raise ContractError("maximum tree depth is outside [10, 14]")
    if not 2 <= len(config["types"]) <= 8:
        raise ContractError("coregionalization requires two through eight types")
    config["correlation_prior"] = prior
    config["correlation_resources"] = resources
    config["numpyro_maximum_tree_depth"] = depth
    config["source_request_sha256"] = source_sha
    return request, config


def seed_for(seed: int, purpose: str) -> int:
    digest = hashlib.sha256(
        f"marklab-numpyro-correlated-multitype-lgcp-v1\0{seed}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def make_model(config: dict[str, Any]) -> Any:
    types = len(config["types"])
    patients = len(config["patient_ids"])
    patterns = len(config["pattern_ids"])
    nodes = len(config["weights"])
    pattern_patients = jnp.asarray(config["pattern_patients"])
    node_patterns = jnp.asarray(config["node_patterns"])
    node_patients = jnp.asarray(config["node_patients"])
    patient_groups = jnp.asarray(config["patient_groups"])
    covariate = jnp.asarray(config["covariate"])
    weights = jnp.asarray(config["weights"])
    node_index = jnp.asarray(config["node_index"])
    type_index = jnp.asarray(config["type_index"])
    observed = jnp.asarray(config["observed"])
    priors = config["priors"]
    distances = []
    selections = []
    for pattern in range(patterns):
        selected = np.flatnonzero(config["node_patterns"] == pattern)
        rows = [config["request"]["nodes"][index] for index in selected]
        coordinates = np.asarray([[row["x_um"], row["y_um"]] for row in rows])
        distances.append(
            jnp.asarray(
                np.linalg.norm(
                    coordinates[:, None, :] - coordinates[None, :, :], axis=2
                )
            )
        )
        selections.append(selected)

    def model() -> None:
        intercept = numpyro.sample(
            "intercept",
            dist.Normal(priors["intercept_mean"], priors["intercept_sd"])
            .expand([types])
            .to_event(1),
        )
        group_effect = numpyro.sample(
            "group_effect",
            dist.Normal(0.0, priors["group_effect_sd"]).expand([types]).to_event(1),
        )
        covariate_effect = numpyro.sample(
            "covariate_effect",
            dist.Normal(0.0, priors["covariate_effect_sd"])
            .expand([types])
            .to_event(1),
        )
        patient_sd = numpyro.sample(
            "patient_sd",
            dist.HalfNormal(priors["patient_sd_scale"]).expand([types]).to_event(1),
        )
        pattern_sd = numpyro.sample(
            "pattern_sd",
            dist.HalfNormal(priors["pattern_sd_scale"]).expand([types]).to_event(1),
        )
        field_scale = numpyro.sample(
            "field_scale",
            dist.HalfNormal(config["correlation_prior"]["marginal_field_scale"])
            .expand([types])
            .to_event(1),
        )
        field_cholesky_correlation = numpyro.sample(
            "field_cholesky_correlation",
            dist.LKJCholesky(
                types,
                concentration=config["correlation_prior"]["lkj_concentration"],
            ),
        )
        correlation_matrix = numpyro.deterministic(
            "correlation_matrix",
            field_cholesky_correlation @ field_cholesky_correlation.T,
        )
        length = numpyro.sample(
            "field_length_scale_um", dist.HalfNormal(config["length_scale"])
        )
        patient_raw = numpyro.sample(
            "patient_raw", dist.Normal(0.0, 1.0).expand([patients, types]).to_event(2)
        )
        pattern_raw = numpyro.sample(
            "pattern_raw", dist.Normal(0.0, 1.0).expand([patterns, types]).to_event(2)
        )
        field_raw = numpyro.sample(
            "field_raw", dist.Normal(0.0, 1.0).expand([types, nodes]).to_event(2)
        )
        patient_effect = numpyro.deterministic(
            "patient_effect", patient_sd[None, :] * patient_raw
        )
        pattern_sums = jnp.zeros((patients, types)).at[pattern_patients].add(pattern_raw)
        pattern_numbers = jnp.zeros(patients).at[pattern_patients].add(1.0)
        pattern_effect = numpyro.deterministic(
            "pattern_effect",
            pattern_sd[None, :]
            * (
                pattern_raw
                - pattern_sums[pattern_patients] / pattern_numbers[pattern_patients, None]
            ),
        )
        blocks = []
        for distance, selected in zip(distances, selections, strict=True):
            scaled = math.sqrt(3.0) * distance / length
            covariance = (1.0 + scaled) * jnp.exp(-scaled)
            covariance += config["kernel_jitter"] * jnp.eye(len(selected))
            independent = field_raw[:, selected] @ jnp.linalg.cholesky(covariance).T
            independent -= independent.mean(axis=1, keepdims=True)
            blocks.append(
                field_scale[:, None] * (field_cholesky_correlation @ independent)
            )
        latent = numpyro.deterministic("latent_effect", jnp.concatenate(blocks, axis=1))
        log_expected = (
            intercept[:, None]
            + group_effect[:, None] * patient_groups[node_patients][None, :]
            + covariate_effect[:, None] * covariate[None, :]
            + patient_effect[node_patients, :].T
            + pattern_effect[node_patterns, :].T
            + latent
            + jnp.log(weights)[None, :]
        )
        expected_matrix = numpyro.deterministic("expected_matrix", jnp.exp(log_expected))
        expected = numpyro.deterministic(
            "expected_count", expected_matrix[type_index, node_index]
        )
        numpyro.sample("observed_count", dist.Poisson(expected), obs=observed)
        del correlation_matrix

    return model


def coregionalization_oracle() -> dict[str, Any]:
    target = np.asarray(
        [[1.0, 0.5, -0.25], [0.5, 1.0, 0.0], [-0.25, 0.0, 1.0]],
        dtype=np.float64,
    )
    lower = np.asarray(
        [
            [1.0, 0.0, 0.0],
            [0.5, math.sqrt(0.75), 0.0],
            [-0.25, 0.125 / math.sqrt(0.75), math.sqrt(11.0 / 12.0)],
        ],
        dtype=np.float64,
    )
    reconstructed = lower @ lower.T
    return {
        "target_correlation_matrix": target.reshape(-1).tolist(),
        "reconstructed_correlation_matrix": reconstructed.reshape(-1).tolist(),
        "maximum_absolute_error": float(np.max(np.abs(target - reconstructed))),
    }


def fit(
    config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str
) -> dict[str, Any]:
    base = load(
        "marklab_numpyro_replicated_arbitrary_window_multitype_lgcp_worker.py",
        "marklab_numpyro_multitype_helpers_for_correlation",
    )
    pymc = load(
        "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py",
        "marklab_pymc_multitype_helpers_for_correlation",
    )
    sampler = MCMC(
        NUTS(
            make_model(config),
            target_accept_prob=config["target_accept"],
            max_tree_depth=config["numpyro_maximum_tree_depth"],
        ),
        num_warmup=config["tune"],
        num_samples=config["draws"],
        num_chains=config["chains"],
        chain_method="sequential",
        progress_bar=False,
    )
    sampler.run(
        jax.random.PRNGKey(seed_for(config["seed"], "nuts")),
        extra_fields=("diverging", "num_steps", "energy"),
    )
    samples = {
        name: np.asarray(value, dtype=np.float64)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    flat = {name: value.reshape((-1, *value.shape[2:])) for name, value in samples.items()}
    patient_effect = flat["patient_sd"][:, None, :] * flat["patient_raw"]
    pattern_centered = flat["pattern_raw"].copy()
    for patient in range(len(config["patient_ids"])):
        selected = config["pattern_patients"] == patient
        pattern_centered[:, selected, :] -= pattern_centered[:, selected, :].mean(
            axis=1, keepdims=True
        )
    pattern_effect = flat["pattern_sd"][:, None, :] * pattern_centered
    latent = np.asarray(samples["latent_effect"]).reshape(
        (-1, len(config["types"]), len(config["weights"]))
    )
    expected = np.asarray(samples["expected_count"]).reshape(
        (-1, len(config["observed"]))
    )
    correlations = np.asarray(samples["correlation_matrix"]).reshape(
        (-1, len(config["types"]), len(config["types"]))
    )
    type_rows = [
        {
            "type_id": identity,
            "intercept": base.summary(flat["intercept"][:, index]),
            "group_effect": base.summary(flat["group_effect"][:, index]),
            "covariate_effect": base.summary(flat["covariate_effect"][:, index]),
            "patient_sd": base.summary(flat["patient_sd"][:, index]),
            "pattern_sd": base.summary(flat["pattern_sd"][:, index]),
        }
        for index, identity in enumerate(config["types"])
    ]
    differences = [
        {
            "type_a": config["types"][left],
            "type_b": config["types"][right],
            "difference": base.summary(
                flat["group_effect"][:, left] - flat["group_effect"][:, right]
            ),
        }
        for left in range(len(config["types"]))
        for right in range(left + 1, len(config["types"]))
    ]
    correlation_rows = [
        {
            "type_a": config["types"][left],
            "type_b": config["types"][right],
            "correlation": base.summary(correlations[:, left, right]),
        }
        for left in range(len(config["types"]))
        for right in range(left + 1, len(config["types"]))
    ]
    predictive_rng = np.random.default_rng(seed_for(config["seed"], "posterior_predictive"))
    replicated = predictive_rng.poisson(expected)
    predictive = []
    for pattern_index, pattern_id in enumerate(config["pattern_ids"]):
        for type_index, type_id in enumerate(config["types"]):
            selected = (
                (config["node_patterns"][config["node_index"]] == pattern_index)
                & (config["type_index"] == type_index)
            )
            observed_nodes = config["observed"][selected]
            replicated_nodes = replicated[:, selected]
            observed_total = int(observed_nodes.sum())
            replicated_totals = replicated_nodes.sum(axis=1)
            observed_variance = float(np.var(observed_nodes))
            replicated_variances = np.var(replicated_nodes, axis=1)
            predictive.append(
                {
                    "pattern_id": pattern_id,
                    "type_id": type_id,
                    "observed_total_count": observed_total,
                    "replicate_count": int(len(replicated_totals)),
                    "replicated_total_count_mean": float(replicated_totals.mean()),
                    "replicated_total_count_sd": float(replicated_totals.std(ddof=1)),
                    "replicated_total_count_interval_lower": float(
                        np.quantile(replicated_totals, 0.025)
                    ),
                    "replicated_total_count_interval_upper": float(
                        np.quantile(replicated_totals, 0.975)
                    ),
                    "total_count_two_sided_tail_probability": pymc.two_sided_tail(
                        replicated_totals, observed_total
                    ),
                    "observed_node_count_variance": observed_variance,
                    "replicated_node_count_variance_mean": float(
                        replicated_variances.mean()
                    ),
                    "node_variance_two_sided_tail_probability": pymc.two_sided_tail(
                        replicated_variances, observed_variance
                    ),
                }
            )
    off_diagonal = np.asarray(
        [
            correlations[:, left, right]
            for left in range(len(config["types"]))
            for right in range(left + 1, len(config["types"]))
        ]
    ).T.reshape((config["chains"], config["draws"], -1))
    diagnostic_samples = {
        name: samples[name]
        for name in (
            "intercept",
            "group_effect",
            "covariate_effect",
            "patient_sd",
            "pattern_sd",
            "field_scale",
            "field_length_scale_um",
            "patient_raw",
            "pattern_raw",
            "field_raw",
        )
    }
    diagnostic_samples["correlation_off_diagonal"] = off_diagonal
    names = list(diagnostic_samples)
    posterior = az.from_dict({"posterior": diagnostic_samples})
    r_hat = float(base.flattened(az.rhat(posterior, var_names=names, method="rank"), names).max())
    bulk = float(base.flattened(az.ess(posterior, var_names=names, method="bulk"), names).min())
    tail = float(base.flattened(az.ess(posterior, var_names=names, method="tail"), names).min())
    mcse_mean = float(
        base.flattened(az.mcse(posterior, var_names=names, method="mean"), names).max()
    )
    mcse_sd = float(
        base.flattened(az.mcse(posterior, var_names=names, method="sd"), names).max()
    )
    extra = sampler.get_extra_fields(group_by_chain=True)
    energy = np.asarray(extra["energy"], dtype=np.float64)
    ebfmi = float(
        np.min(np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1))
    )
    divergences = int(np.asarray(extra["diverging"]).sum())
    depth_hits = int(
        (
            np.asarray(extra["num_steps"])
            >= 2 ** config["numpyro_maximum_tree_depth"] - 1
        ).sum()
    )
    constraints = bool(
        all(
            np.max(
                np.abs(
                    pattern_effect[:, config["pattern_patients"] == patient, :].sum(axis=1)
                )
            )
            < 1e-10
            for patient in range(len(config["patient_ids"]))
        )
        and all(
            np.max(
                np.abs(latent[:, :, config["node_patterns"] == pattern].sum(axis=2))
            )
            < 1e-10
            for pattern in range(len(config["pattern_ids"]))
        )
        and np.all(np.diagonal(correlations, axis1=1, axis2=2) > 0.999999999)
        and np.all(np.linalg.eigvalsh(correlations) > 0.0)
    )
    finite = bool(
        all(
            np.isfinite(value).all()
            for value in (patient_effect, pattern_effect, latent, expected, correlations, replicated)
        )
    )
    policy = config["policy"]
    complete = bool(
        finite
        and constraints
        and r_hat <= policy["maximum_r_hat"]
        and bulk >= policy["minimum_bulk_ess"]
        and tail >= policy["minimum_tail_ess"]
        and ebfmi >= policy["minimum_ebfmi"]
        and divergences <= policy["maximum_divergences"]
        and depth_hits <= policy["maximum_tree_depth_hits"]
    )
    return {
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
        "input_sha256": config["input_sha"],
        "request_sha256": request_sha,
        "source_request_sha256": config["source_request_sha256"],
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "field_length_scale_um": base.summary(flat["field_length_scale_um"]),
        "type_field_scales": [
            {
                "type_id": identity,
                "scale": base.summary(flat["field_scale"][:, index]),
            }
            for index, identity in enumerate(config["types"])
        ],
        "cross_type_correlations": correlation_rows,
        "posterior_mean_correlation_matrix": correlations.mean(axis=0).reshape(-1).tolist(),
        "type_posteriors": type_rows,
        "group_effect_differences": differences,
        "pattern_type_posterior_predictive": predictive,
        "diagnostics": {
            "prior_predictive_finite": True,
            "posterior_finite": finite,
            "r_hat": r_hat,
            "ess_bulk": bulk,
            "ess_tail": tail,
            "mcse_mean": mcse_mean,
            "mcse_sd": mcse_sd,
            "minimum_ebfmi": ebfmi,
            "divergences": divergences,
            "max_tree_depth_hits": depth_hits,
            "constraints_valid": constraints,
            "identifiability_checks_passed": constraints,
        },
        "coregionalization_oracle": coregionalization_oracle(),
        "coregionalization_work": config["correlation_resources"]["coregionalization_work"],
    }


def main() -> int:
    if (
        numpyro.__version__ != NUMPYRO_VERSION
        or jax.__version__ != JAX_VERSION
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("NumPyro, JAX, or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    pymc_inferred_sha = hashlib.sha256(
        script.with_name(
            "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_worker.py"
        ).read_bytes()
    ).hexdigest()
    pymc_fixed_sha = hashlib.sha256(
        script.with_name(
            "marklab_pymc_replicated_arbitrary_window_multitype_lgcp_worker.py"
        ).read_bytes()
    ).hexdigest()
    raw = sys.stdin.buffer.read(64 * 1_048_576 + 1)
    if not raw or len(raw) > 64 * 1_048_576:
        raise ContractError("request size is invalid")
    request_sha = hashlib.sha256(raw).hexdigest()
    _, config = validate(
        json.loads(raw), lock_sha, worker_sha, pymc_inferred_sha, pymc_fixed_sha
    )
    output = json.dumps(
        fit(config, request_sha, lock_sha, worker_sha),
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    ) + "\n"
    if len(output.encode()) > config["correlation_resources"]["maximum_output_bytes"]:
        raise ContractError("result exceeds output ceiling")
    sys.stdout.write(output)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(
            f"marklab correlated NumPyro multitype LGCP failed: {type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
