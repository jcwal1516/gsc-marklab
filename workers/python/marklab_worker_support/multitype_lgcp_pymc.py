"""PyMC model and fit shared by the fixed- and inferred-kernel multitype LGCP workers."""

from __future__ import annotations

import math
from collections.abc import Callable
from typing import Any

# Configure PyTensor before PyMC is imported.
from . import pytensor_config  # noqa: F401

# isort: split
import numpy as np
import pymc as pm
import pytensor.tensor as pt

from . import multitype_lgcp as lgcp
from .diagnostics import mcse_extremes, pymc_sampler_diagnostics, rank_convergence
from .protocol import ContractError

KERNEL_PARAMETERS = ["field_amplitude", "field_length_scale_um"]


def fixed_kernel_latent(config: dict[str, Any], field_raw: Any) -> Any:
    """Latent fields ``field_raw @ L.T`` for the request's block Cholesky factor ``L``."""
    patterns = len(config["pattern_ids"])
    latent_uncentered = field_raw @ config["cholesky"].T
    latent_pattern_mean = pt.stack(
        [
            pt.mean(
                latent_uncentered[:, np.flatnonzero(config["node_patterns"] == pattern)],
                axis=1,
            )
            for pattern in range(patterns)
        ],
        axis=1,
    )
    return latent_uncentered - latent_pattern_mean[:, config["node_patterns"]]


def inferred_kernel_latent(
    config: dict[str, Any], amplitude: Any, length: Any, field_raw: Any
) -> Any:
    """Latent fields with a per-pattern Matern-3/2 covariance of inferred amplitude and length."""
    blocks = []
    for selected, distance in lgcp.pattern_distances(config):
        scaled = math.sqrt(3.0) * distance / length
        covariance = amplitude**2 * (1.0 + scaled) * pt.exp(-scaled)
        covariance += config["kernel_jitter"] * np.eye(len(selected))
        block = field_raw[:, selected] @ pt.linalg.cholesky(covariance).T
        blocks.append(block - pt.mean(block, axis=1, keepdims=True))
    return pt.concatenate(blocks, axis=1)


def build_model(config: dict[str, Any], *, inferred_kernel: bool) -> pm.Model:
    """Build the replicated multitype LGCP (see ``multitype_lgcp``) in PyMC.

    Per type: ``intercept ~ Normal(intercept_mean, intercept_sd)``, ``group_effect ~
    Normal(0, group_effect_sd)``, ``covariate_effect ~ Normal(0, covariate_effect_sd)``,
    ``patient_sd ~ HalfNormal(patient_sd_scale)``, ``pattern_sd ~ HalfNormal(pattern_sd_scale)``.
    Patient, pattern, and field effects are non-centred: standard-normal raw values scaled by
    their SDs, with pattern raw values centred within each patient and latent fields centred
    within each pattern. The latent field is ``field_raw @ L.T`` for the request's fixed block
    Cholesky factor ``L``, or, with ``inferred_kernel``, uses a Matern-3/2 kernel per pattern
    with ``field_amplitude ~ HalfNormal(amplitude_scale)`` and ``field_length_scale_um ~
    HalfNormal(length_scale)``. Counts are Poisson with the quadrature-weighted intensity.
    """
    priors = config["priors"]
    types = len(config["types"])
    patients = len(config["patient_ids"])
    patterns = len(config["pattern_ids"])
    nodes = len(config["weights"])
    node_patients = config["node_patients"]
    node_patterns = config["node_patterns"]
    pattern_patients = config["pattern_patients"]
    model = pm.Model()
    with model:
        intercept = pm.Normal(
            "intercept", priors["intercept_mean"], priors["intercept_sd"], shape=types
        )
        group_effect = pm.Normal("group_effect", 0.0, priors["group_effect_sd"], shape=types)
        covariate_effect = pm.Normal(
            "covariate_effect", 0.0, priors["covariate_effect_sd"], shape=types
        )
        patient_sd = pm.HalfNormal("patient_sd", priors["patient_sd_scale"], shape=types)
        pattern_sd = pm.HalfNormal("pattern_sd", priors["pattern_sd_scale"], shape=types)
        if inferred_kernel:
            field_amplitude = pm.HalfNormal("field_amplitude", config["amplitude_scale"])
            field_length_scale_um = pm.HalfNormal("field_length_scale_um", config["length_scale"])
        patient_raw = pm.Normal("patient_raw", 0.0, 1.0, shape=(patients, types))
        pattern_raw = pm.Normal("pattern_raw", 0.0, 1.0, shape=(patterns, types))
        field_raw = pm.Normal("field_raw", 0.0, 1.0, shape=(types, nodes))
        patient_effect = pm.Deterministic("patient_effect", patient_sd[None, :] * patient_raw)
        patient_pattern_mean = pt.stack(
            [
                pt.mean(pattern_raw[np.flatnonzero(pattern_patients == patient), :], axis=0)
                for patient in range(patients)
            ]
        )
        pattern_effect = pm.Deterministic(
            "pattern_effect",
            pattern_sd[None, :] * (pattern_raw - patient_pattern_mean[pattern_patients, :]),
        )
        if inferred_kernel:
            latent = inferred_kernel_latent(
                config, field_amplitude, field_length_scale_um, field_raw
            )
        else:
            latent = fixed_kernel_latent(config, field_raw)
        latent_effect = pm.Deterministic("latent_effect", latent)
        log_expected = (
            intercept[:, None]
            + group_effect[:, None] * config["patient_groups"][node_patients][None, :]
            + covariate_effect[:, None] * config["covariate"][None, :]
            + patient_effect[node_patients, :].T
            + pattern_effect[node_patterns, :].T
            + latent_effect
            + np.log(config["weights"])[None, :]
        )
        expected_matrix = pm.Deterministic("expected_matrix", pm.math.exp(log_expected))
        expected_count = pm.Deterministic(
            "expected_count", expected_matrix[config["type_index"], config["node_index"]]
        )
        pm.Poisson("observed_count", mu=expected_count, observed=config["observed"])
    return model


def prior_is_finite(
    config: dict[str, Any], model: pm.Model, seed_for: Callable[[int, str], int]
) -> bool:
    """Whether prior-predictive rates are finite and positive and allow Poisson count draws."""
    try:
        with model:
            prior = pm.sample_prior_predictive(
                draws=int(config["policy"]["prior_predictive_draws"]),
                var_names=["expected_count"],
                random_seed=seed_for(config["seed"], "prior"),
                return_inferencedata=True,
            )
    except (FloatingPointError, OverflowError, ValueError):
        return False
    if "prior" not in prior:
        return False
    expected = np.asarray(prior["prior"]["expected_count"].values, dtype=np.float64)
    if not np.isfinite(expected).all() or np.any(expected <= 0.0):
        return False
    try:
        counts = np.random.default_rng(seed_for(config["seed"], "prior_counts")).poisson(expected)
    except ValueError:
        return False
    return bool(np.isfinite(counts).all() and np.all(counts >= 0))


def fit(
    config: dict[str, Any],
    *,
    inferred_kernel: bool,
    maximum_tree_depth: int,
    seed_for: Callable[[int, str], int],
) -> tuple[dict[str, np.ndarray], dict[str, Any]]:
    """Check the prior predictive, sample by NUTS, and summarize the posterior.

    Returns the draw-major posterior arrays and the result fields common to both kernels.
    """
    kernel = KERNEL_PARAMETERS if inferred_kernel else []
    model = build_model(config, inferred_kernel=inferred_kernel)
    prior_finite = prior_is_finite(config, model, seed_for)
    if not prior_finite:
        raise ContractError("prior predictive contains invalid rates or conditional Poisson counts")
    with model:
        trace = pm.sample(
            draws=config["draws"],
            tune=config["tune"],
            chains=config["chains"],
            cores=1,
            blas_cores=1,
            target_accept=config["target_accept"],
            random_seed=config["seed"],
            progressbar=False,
            quiet=True,
            compute_convergence_checks=False,
            return_inferencedata=True,
            nuts={"max_treedepth": maximum_tree_depth},
        )
    monitored = [
        "intercept",
        "group_effect",
        "covariate_effect",
        "patient_sd",
        "pattern_sd",
        *kernel,
        "patient_raw",
        "pattern_raw",
        "field_raw",
    ]
    r_hat, bulk, tail = rank_convergence(trace, monitored)
    mcse_mean, mcse_sd = mcse_extremes(trace, monitored)
    ebfmi, divergences, depth_hits = pymc_sampler_diagnostics(trace.sample_stats)
    posterior = trace.posterior
    summarized = [
        "intercept",
        "group_effect",
        "covariate_effect",
        "patient_sd",
        "pattern_sd",
        *kernel,
        "patient_effect",
        "pattern_effect",
        "latent_effect",
        "expected_count",
    ]
    flat = {
        name: np.asarray(posterior[name], dtype=np.float64).reshape(
            (-1, *np.asarray(posterior[name]).shape[2:])
        )
        for name in summarized
    }
    predictive_rng = np.random.default_rng(seed_for(config["seed"], "posterior_predictive"))
    replicated = predictive_rng.poisson(flat["expected_count"])
    constraints = bool(
        lgcp.centering_constraints_hold(config, flat["pattern_effect"], flat["latent_effect"])
    )
    posterior_finite = bool(all(np.isfinite(value).all() for value in flat.values()))
    policy = config["policy"]
    complete = bool(
        prior_finite
        and posterior_finite
        and constraints
        and r_hat <= policy["maximum_r_hat"]
        and bulk >= policy["minimum_bulk_ess"]
        and tail >= policy["minimum_tail_ess"]
        and ebfmi >= policy["minimum_ebfmi"]
        and divergences <= policy["maximum_divergences"]
        and depth_hits <= policy["maximum_tree_depth_hits"]
    )
    fields = {
        "input_sha256": config["input_sha"],
        "fit_state": "complete" if complete else "nonconverged",
        "sampling": {
            "chains": config["chains"],
            "tune_per_chain": config["tune"],
            "draws_per_chain": config["draws"],
            "completed_draws": config["chains"] * config["draws"],
        },
        "type_posteriors": lgcp.type_rows(config, flat),
        "group_effect_differences": lgcp.group_effect_differences(config, flat["group_effect"]),
        "patient_type_effects": lgcp.owner_type_effects(
            config["patient_ids"], config["types"], flat["patient_effect"], config["chains"]
        ),
        "pattern_type_effects": lgcp.owner_type_effects(
            config["pattern_ids"], config["types"], flat["pattern_effect"], config["chains"]
        ),
        "node_type_posteriors": lgcp.node_type_rows(
            config, flat["latent_effect"], flat["expected_count"]
        ),
        "pattern_type_posterior_predictive": lgcp.pattern_type_predictive(config, replicated),
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
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
    }
    return flat, fields
