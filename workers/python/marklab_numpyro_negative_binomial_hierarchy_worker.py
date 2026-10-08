#!/usr/bin/env python3
"""Pinned NumPyro worker for exposure-aware negative-binomial hierarchies."""

from __future__ import annotations

import hashlib
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


REQUEST_FORMAT = "marklab.numpyro_negative_binomial_hierarchy_request"
RESULT_FORMAT = "marklab.negative_binomial_hierarchy"
NUMPYRO_VERSION = "0.21.0"
JAX_VERSION = "0.11.1"
MAXIMUM_INPUT_BYTES = 16 * 1024 * 1024
MAXIMUM_OUTPUT_BYTES = 1024 * 1024
MAXIMUM_DRAW_OBSERVATION_PRODUCTS = 16_000_000
MAXIMUM_TOTAL_ITERATIONS = 100_000


class ContractError(ValueError):
    pass


def exact_object(value: Any, keys: set[str], path: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(
            f"{path} fields differ: missing={sorted(keys - actual)}, "
            f"unknown={sorted(actual - keys)}"
        )
    return value


def exact(value: Any, expected: Any, path: str) -> None:
    if value != expected:
        raise ContractError(f"{path} must equal {expected!r}")


def finite_number(value: Any, path: str, *, positive: bool = False) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{path} must be a finite number")
    result = float(value)
    if not math.isfinite(result) or (positive and result <= 0.0):
        qualifier = "positive " if positive else ""
        raise ContractError(f"{path} must be a finite {qualifier}number")
    return result


def integer(value: Any, path: str, lower: int, upper: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not lower <= value <= upper:
        raise ContractError(f"{path} must be an integer in [{lower}, {upper}]")
    return value


def identifier(value: Any, path: str) -> str:
    if (
        not isinstance(value, str)
        or not value
        or len(value) > 128
        or value.strip() != value
    ):
        raise ContractError(f"{path} must be a nonempty trimmed identifier of at most 128 characters")
    return value


def lower_hex_sha256(value: Any, path: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ContractError(f"{path} must be a lowercase SHA-256 digest")
    return value


def _compensated_sum(values: list[float] | np.ndarray) -> float:
    """Neumaier sum mirrored by the typed request compiler."""
    total = 0.0
    correction = 0.0
    for raw_value in values:
        value = float(raw_value)
        next_total = total + value
        if abs(total) >= abs(value):
            correction += (total - next_total) + value
        else:
            correction += (value - next_total) + total
        total = next_total
    return total + correction


def compile_design(observations: list[dict[str, Any]]) -> dict[str, Any]:
    patient_ids = sorted({row["patient_id"] for row in observations})
    slide_ids = sorted({row["slide_id"] for row in observations})
    patient_lookup = {identity: index for index, identity in enumerate(patient_ids)}
    slide_lookup = {identity: index for index, identity in enumerate(slide_ids)}
    patient_index = [patient_lookup[row["patient_id"]] for row in observations]
    slide_index = [slide_lookup[row["slide_id"]] for row in observations]
    patient_predictor_means = []
    centered = np.empty(len(observations), dtype=np.float64)
    for patient, patient_id in enumerate(patient_ids):
        selected = [index for index, value in enumerate(patient_index) if value == patient]
        origin = observations[selected[0]]["predictor"]
        offsets = [observations[index]["predictor"] - origin for index in selected]
        mean_offset = _compensated_sum(offsets) / len(selected)
        patient_predictor_means.append(origin + mean_offset)
        centered[selected] = [offset - mean_offset for offset in offsets]
    maximum_absolute = float(np.max(np.abs(centered)))
    if not math.isfinite(maximum_absolute) or maximum_absolute <= 0.0:
        raise ContractError("within-patient predictor RMS must be positive and finite")
    scale = maximum_absolute * math.sqrt(
        _compensated_sum((centered / maximum_absolute) ** 2) / len(observations)
    )
    if not math.isfinite(scale) or scale <= 0.0:
        raise ContractError("within-patient predictor RMS must be positive and finite")
    return {
        "patient_ids": patient_ids,
        "slide_ids": slide_ids,
        "patient_index": patient_index,
        "slide_index": slide_index,
        "patient_predictor_means": patient_predictor_means,
        "within_predictor": (centered / scale).tolist(),
        "within_predictor_sd": scale,
    }


def _validate_observations(raw: Any) -> list[dict[str, Any]]:
    if not isinstance(raw, list) or not 64 <= len(raw) <= 512:
        raise ContractError("spec.observations must contain 64 to 512 rows")
    observations = []
    for index, raw_row in enumerate(raw):
        row = exact_object(
            raw_row,
            {"patient_id", "slide_id", "roi_id", "count", "area", "predictor"},
            f"spec.observations[{index}]",
        )
        observations.append(
            {
                "patient_id": identifier(row["patient_id"], f"observations[{index}].patient_id"),
                "slide_id": identifier(row["slide_id"], f"observations[{index}].slide_id"),
                "roi_id": identifier(row["roi_id"], f"observations[{index}].roi_id"),
                "count": integer(row["count"], f"observations[{index}].count", 0, 2**32 - 1),
                "area": finite_number(row["area"], f"observations[{index}].area", positive=True),
                "predictor": finite_number(row["predictor"], f"observations[{index}].predictor"),
            }
        )
    if observations != sorted(
        observations, key=lambda row: (row["patient_id"], row["slide_id"], row["roi_id"])
    ):
        raise ContractError("spec.observations are not in canonical patient/slide/ROI order")
    roi_ids = [row["roi_id"] for row in observations]
    if len(set(roi_ids)) != len(roi_ids):
        raise ContractError("ROI IDs must be globally unique")
    patients = {row["patient_id"] for row in observations}
    slides = {row["slide_id"] for row in observations}
    if not 8 <= len(patients) <= 64 or not 16 <= len(slides) <= 128:
        raise ContractError("design requires 8-64 patients and 16-128 slides")
    slide_parent: dict[str, str] = {}
    patient_slides = {patient: set() for patient in patients}
    slide_rows = {slide: [] for slide in slides}
    for row in observations:
        parent = slide_parent.setdefault(row["slide_id"], row["patient_id"])
        if parent != row["patient_id"]:
            raise ContractError("a slide cannot belong to multiple patients")
        patient_slides[row["patient_id"]].add(row["slide_id"])
        slide_rows[row["slide_id"]].append(row)
    if any(len(patient_slides[patient]) < 2 for patient in patients):
        raise ContractError("every patient requires at least two slides")
    for slide_id, rows in slide_rows.items():
        if len(rows) < 4:
            raise ContractError(f"slide {slide_id!r} requires at least four ROIs")
        low = min(row["predictor"] for row in rows)
        high = max(row["predictor"] for row in rows)
        if not high > low:
            raise ContractError(f"slide {slide_id!r} lacks identifiable predictor variation")
    return observations


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = exact_object(
        request,
        {"format", "version", "backend", "jax_version", "spec", "design", "diagnostic_policy"},
        "request",
    )
    exact(request["format"], REQUEST_FORMAT, "request.format")
    exact(request["version"], 1, "request.version")
    backend = exact_object(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "request.backend",
    )
    exact(backend["name"], "numpyro", "request.backend.name")
    exact(backend["version"], NUMPYRO_VERSION, "request.backend.version")
    exact(backend["python_version"], "3.12", "request.backend.python_version")
    exact(
        lower_hex_sha256(backend["environment_lock_sha256"], "request.backend.environment_lock_sha256"),
        lock_digest,
        "request.backend.environment_lock_sha256",
    )
    exact(
        lower_hex_sha256(backend["worker_sha256"], "request.backend.worker_sha256"),
        worker_digest,
        "request.backend.worker_sha256",
    )
    exact(request["jax_version"], JAX_VERSION, "request.jax_version")
    spec = exact_object(
        request["spec"],
        {
            "area_unit", "predictor_name", "predictor_unit", "observations", "priors",
            "sampling", "timeout_seconds", "maximum_draw_observation_products",
        },
        "request.spec",
    )
    area_unit = identifier(spec["area_unit"], "spec.area_unit")
    predictor_name = identifier(spec["predictor_name"], "spec.predictor_name")
    predictor_unit = identifier(spec["predictor_unit"], "spec.predictor_unit")
    observations = _validate_observations(spec["observations"])
    priors_raw = exact_object(
        spec["priors"],
        {
            "intercept_mean", "intercept_sd", "slope_sd", "log_dispersion_mean",
            "log_dispersion_sd", "patient_intercept_sd_scale", "patient_slope_sd_scale",
            "slide_sd_scale",
        },
        "spec.priors",
    )
    priors = {
        "intercept_mean": finite_number(priors_raw["intercept_mean"], "priors.intercept_mean"),
        "intercept_sd": finite_number(priors_raw["intercept_sd"], "priors.intercept_sd", positive=True),
        "slope_sd": finite_number(priors_raw["slope_sd"], "priors.slope_sd", positive=True),
        "log_dispersion_mean": finite_number(
            priors_raw["log_dispersion_mean"], "priors.log_dispersion_mean"
        ),
        "log_dispersion_sd": finite_number(
            priors_raw["log_dispersion_sd"], "priors.log_dispersion_sd", positive=True
        ),
        "patient_intercept_sd_scale": finite_number(
            priors_raw["patient_intercept_sd_scale"], "priors.patient_intercept_sd_scale", positive=True
        ),
        "patient_slope_sd_scale": finite_number(
            priors_raw["patient_slope_sd_scale"], "priors.patient_slope_sd_scale", positive=True
        ),
        "slide_sd_scale": finite_number(
            priors_raw["slide_sd_scale"], "priors.slide_sd_scale", positive=True
        ),
    }
    sampling_raw = exact_object(
        spec["sampling"],
        {"chains", "tune_per_chain", "draws_per_chain", "target_accept", "seed"},
        "spec.sampling",
    )
    sampling = {
        "chains": integer(sampling_raw["chains"], "sampling.chains", 2, 8),
        "tune_per_chain": integer(
            sampling_raw["tune_per_chain"], "sampling.tune_per_chain", 100, 100_000
        ),
        "draws_per_chain": integer(
            sampling_raw["draws_per_chain"], "sampling.draws_per_chain", 100, 100_000
        ),
        "target_accept": finite_number(sampling_raw["target_accept"], "sampling.target_accept"),
        "seed": integer(sampling_raw["seed"], "sampling.seed", 0, 2**64 - 1),
    }
    if not 0.5 <= sampling["target_accept"] < 1.0:
        raise ContractError("sampling.target_accept must be in [0.5, 1)")
    if sampling["chains"] * (
        sampling["tune_per_chain"] + sampling["draws_per_chain"]
    ) > MAXIMUM_TOTAL_ITERATIONS:
        raise ContractError("sampling exceeds 100000 total NUTS iterations")
    timeout_seconds = integer(spec["timeout_seconds"], "spec.timeout_seconds", 1, 3_600)
    maximum_products = integer(
        spec["maximum_draw_observation_products"],
        "spec.maximum_draw_observation_products",
        1,
        MAXIMUM_DRAW_OBSERVATION_PRODUCTS,
    )
    posterior_products = sampling["chains"] * sampling["draws_per_chain"] * len(observations)
    policy_raw = exact_object(
        request["diagnostic_policy"],
        {
            "prior_predictive_draws", "maximum_r_hat", "minimum_bulk_ess",
            "minimum_tail_ess", "minimum_ebfmi", "maximum_divergences",
            "maximum_tree_depth_hits", "maximum_tree_depth",
        },
        "request.diagnostic_policy",
    )
    policy = {
        "prior_predictive_draws": integer(
            policy_raw["prior_predictive_draws"], "policy.prior_predictive_draws", 1, 10_000
        ),
        "maximum_r_hat": finite_number(policy_raw["maximum_r_hat"], "policy.maximum_r_hat"),
        "minimum_bulk_ess": finite_number(
            policy_raw["minimum_bulk_ess"], "policy.minimum_bulk_ess", positive=True
        ),
        "minimum_tail_ess": finite_number(
            policy_raw["minimum_tail_ess"], "policy.minimum_tail_ess", positive=True
        ),
        "minimum_ebfmi": finite_number(policy_raw["minimum_ebfmi"], "policy.minimum_ebfmi"),
        "maximum_divergences": integer(
            policy_raw["maximum_divergences"], "policy.maximum_divergences", 0, 2**64 - 1
        ),
        "maximum_tree_depth_hits": integer(
            policy_raw["maximum_tree_depth_hits"], "policy.maximum_tree_depth_hits", 0, 2**64 - 1
        ),
        "maximum_tree_depth": integer(
            policy_raw["maximum_tree_depth"], "policy.maximum_tree_depth", 1, 32
        ),
    }
    fixed_policy = {
        "prior_predictive_draws": 500,
        "maximum_r_hat": 1.01,
        "minimum_bulk_ess": 400.0,
        "minimum_tail_ess": 400.0,
        "minimum_ebfmi": 0.3,
        "maximum_divergences": 0,
        "maximum_tree_depth_hits": 0,
        "maximum_tree_depth": 10,
    }
    for name, expected in fixed_policy.items():
        exact(policy[name], expected, f"diagnostic_policy.{name}")
    products = posterior_products + policy["prior_predictive_draws"] * len(observations)
    if products > maximum_products:
        raise ContractError("predictive draw-observation products exceed the declared ceiling")
    compiled = compile_design(observations)
    design = exact_object(
        request["design"],
        {
            "patient_ids", "slide_ids", "patient_index", "slide_index",
            "patient_predictor_means", "within_predictor", "within_predictor_sd",
        },
        "request.design",
    )
    if design != compiled:
        raise ContractError("request.design does not match independent predictor compilation")
    return {
        "area_unit": area_unit,
        "predictor_name": predictor_name,
        "predictor_unit": predictor_unit,
        "observations": observations,
        "priors": priors,
        "sampling": sampling,
        "timeout_seconds": timeout_seconds,
        "maximum_draw_observation_products": maximum_products,
        "design": compiled,
        "policy": policy,
    }


def seed_for(seed: int, purpose: str) -> int:
    digest = hashlib.sha256(
        f"marklab-numpyro-negative-binomial-hierarchy-v1\0{seed}\0{purpose}".encode()
    ).digest()
    return int.from_bytes(digest[:4], "little")


def model(
    counts: jnp.ndarray,
    areas: jnp.ndarray,
    within_predictor: jnp.ndarray,
    patient_index: jnp.ndarray,
    slide_index: jnp.ndarray,
    patient_template: jnp.ndarray,
    slide_template: jnp.ndarray,
    priors: dict[str, float],
) -> None:
    alpha = numpyro.sample(
        "alpha", dist.Normal(priors["intercept_mean"], priors["intercept_sd"])
    )
    beta = numpyro.sample("beta", dist.Normal(0.0, priors["slope_sd"]))
    log_dispersion = numpyro.sample(
        "log_dispersion",
        dist.Normal(priors["log_dispersion_mean"], priors["log_dispersion_sd"]),
    )
    patient_intercept_sd = numpyro.sample(
        "patient_intercept_sd", dist.HalfNormal(priors["patient_intercept_sd_scale"])
    )
    patient_slope_sd = numpyro.sample(
        "patient_slope_sd", dist.HalfNormal(priors["patient_slope_sd_scale"])
    )
    slide_sd = numpyro.sample("slide_sd", dist.HalfNormal(priors["slide_sd_scale"]))
    patient_intercept_z = numpyro.sample(
        "patient_intercept_z",
        dist.Normal(jnp.zeros_like(patient_template), 1.0).to_event(1),
    )
    patient_slope_z = numpyro.sample(
        "patient_slope_z", dist.Normal(jnp.zeros_like(patient_template), 1.0).to_event(1)
    )
    slide_z = numpyro.sample(
        "slide_z", dist.Normal(jnp.zeros_like(slide_template), 1.0).to_event(1)
    )
    patient_intercept = numpyro.deterministic(
        "patient_intercept", patient_intercept_sd * patient_intercept_z
    )
    patient_slope = numpyro.deterministic("patient_slope", patient_slope_sd * patient_slope_z)
    slide_intercept = numpyro.deterministic("slide_intercept", slide_sd * slide_z)
    eta = (
        alpha
        + beta * within_predictor
        + patient_intercept[patient_index]
        + patient_slope[patient_index] * within_predictor
        + slide_intercept[slide_index]
    )
    log_expected_count = jnp.log(areas) + eta
    expected_count = numpyro.deterministic("expected_count", jnp.exp(log_expected_count))
    dispersion = numpyro.deterministic("dispersion", jnp.exp(log_dispersion))
    log_denominator = jnp.logaddexp(log_dispersion, log_expected_count)
    count_log_probability = (
        jax.scipy.special.gammaln(counts + dispersion)
        - jax.scipy.special.gammaln(dispersion)
        - jax.scipy.special.gammaln(counts + 1.0)
        + dispersion * (log_dispersion - log_denominator)
        + counts * (log_expected_count - log_denominator)
    )
    numpyro.factor("count_log_likelihood", jnp.sum(count_log_probability))


def build_sampler(config: dict[str, Any]) -> MCMC:
    """Build a reusable sampler; repeated equal-shape runs reuse JAX compilation."""
    return MCMC(
        NUTS(
            model,
            target_accept_prob=config["sampling"]["target_accept"],
            max_tree_depth=config["policy"]["maximum_tree_depth"],
        ),
        num_warmup=config["sampling"]["tune_per_chain"],
        num_samples=config["sampling"]["draws_per_chain"],
        num_chains=config["sampling"]["chains"],
        chain_method="sequential",
        progress_bar=False,
        jit_model_args=True,
    )


def summary(values: np.ndarray) -> dict[str, float]:
    flat = np.asarray(values, dtype=np.float64).reshape(-1)
    return {
        "mean": float(flat.mean()),
        "sd": float(flat.std(ddof=1)),
        "interval_lower": float(np.quantile(flat, 0.025)),
        "interval_upper": float(np.quantile(flat, 0.975)),
    }


def _flattened(tree: Any, names: list[str]) -> np.ndarray:
    return np.concatenate([np.asarray(tree[name].values).reshape(-1) for name in names])


def _chain_aware_mcse_mean(values: np.ndarray) -> float | None:
    values = np.asarray(values, dtype=np.float64)
    if np.all(values == values.flat[0]):
        return None
    data = az.from_dict({"posterior": {"indicator": values}})
    result = float(
        np.asarray(az.mcse(data, var_names=["indicator"], method="mean")["indicator"])
    )
    return result if math.isfinite(result) and result >= 0.0 else None


def _draw_negative_binomial(
    rng: np.random.Generator, means: np.ndarray, dispersion: np.ndarray
) -> np.ndarray:
    probability = dispersion / (dispersion + means)
    return rng.negative_binomial(dispersion, probability)


def _prior_predictive(config: dict[str, Any]) -> tuple[dict[str, Any], bool]:
    draw_count = config["policy"]["prior_predictive_draws"]
    rng = np.random.default_rng(seed_for(config["sampling"]["seed"], "prior_predictive"))
    priors = config["priors"]
    patient_count = len(config["design"]["patient_ids"])
    slide_count = len(config["design"]["slide_ids"])
    alpha = rng.normal(priors["intercept_mean"], priors["intercept_sd"], draw_count)
    beta = rng.normal(0.0, priors["slope_sd"], draw_count)
    log_dispersion = rng.normal(
        priors["log_dispersion_mean"], priors["log_dispersion_sd"], draw_count
    )
    patient_intercept_sd = np.abs(
        rng.normal(0.0, priors["patient_intercept_sd_scale"], draw_count)
    )
    patient_slope_sd = np.abs(
        rng.normal(0.0, priors["patient_slope_sd_scale"], draw_count)
    )
    slide_sd = np.abs(rng.normal(0.0, priors["slide_sd_scale"], draw_count))
    patient_intercept = patient_intercept_sd[:, None] * rng.normal(
        size=(draw_count, patient_count)
    )
    patient_slope = patient_slope_sd[:, None] * rng.normal(size=(draw_count, patient_count))
    slide_intercept = slide_sd[:, None] * rng.normal(size=(draw_count, slide_count))
    patient_index = np.asarray(config["design"]["patient_index"], dtype=np.int32)
    slide_index = np.asarray(config["design"]["slide_index"], dtype=np.int32)
    within = np.asarray(config["design"]["within_predictor"])
    areas = np.asarray([row["area"] for row in config["observations"]])
    eta = (
        alpha[:, None]
        + beta[:, None] * within
        + patient_intercept[:, patient_index]
        + patient_slope[:, patient_index] * within
        + slide_intercept[:, slide_index]
    )
    with np.errstate(over="ignore", invalid="ignore"):
        means = areas * np.exp(eta)
        dispersion = np.exp(log_dispersion)[:, None]
    finite = bool(np.isfinite(means).all() and np.isfinite(dispersion).all())
    if not finite:
        raise ContractError("prior predictive expected counts are nonfinite")
    try:
        replicated = _draw_negative_binomial(rng, means, dispersion)
    except (OverflowError, ValueError) as error:
        raise ContractError("prior predictive counts cannot be represented") from error
    patient_rates = []
    for patient, patient_id in enumerate(config["design"]["patient_ids"]):
        selected = patient_index == patient
        rates = replicated[:, selected].sum(axis=1) / areas[selected].sum()
        rate_summary = summary(rates)
        patient_rates.append({
            "patient_id": patient_id,
            "mean": rate_summary["mean"],
            "sd": rate_summary["sd"],
        })
    zero_fraction = np.mean(replicated == 0, axis=1)
    return {
        "draws": draw_count,
        "patient_rates": patient_rates,
        "zero_fraction_mean": float(zero_fraction.mean()),
        "zero_fraction_sd": float(zero_fraction.std(ddof=1)),
    }, bool(np.isfinite(replicated).all())


def fit(
    config: dict[str, Any],
    request_sha256: str,
    lock_digest: str,
    worker_digest: str,
    *,
    sampler: MCMC | None = None,
    return_samples: bool = False,
) -> dict[str, Any] | tuple[dict[str, Any], dict[str, np.ndarray]]:
    observations = config["observations"]
    design = config["design"]
    sampling = config["sampling"]
    areas = np.asarray([row["area"] for row in observations], dtype=np.float64)
    counts = np.asarray([row["count"] for row in observations], dtype=np.int64)
    predictor = np.asarray([row["predictor"] for row in observations], dtype=np.float64)
    within = np.asarray(design["within_predictor"], dtype=np.float64)
    patient_index = np.asarray(design["patient_index"], dtype=np.int32)
    slide_index = np.asarray(design["slide_index"], dtype=np.int32)
    arguments = {
        "areas": jnp.asarray(areas),
        "within_predictor": jnp.asarray(within),
        "patient_index": jnp.asarray(patient_index),
        "slide_index": jnp.asarray(slide_index),
        "patient_template": jnp.zeros(len(design["patient_ids"]), dtype=jnp.float64),
        "slide_template": jnp.zeros(len(design["slide_ids"]), dtype=jnp.float64),
        "priors": config["priors"],
    }
    prior_predictive, prior_finite = _prior_predictive(config)
    sampler = build_sampler(config) if sampler is None else sampler
    sampler.run(
        jax.random.PRNGKey(seed_for(sampling["seed"], "nuts")),
        counts=jnp.asarray(counts),
        **arguments,
        extra_fields=("diverging", "num_steps", "energy"),
    )
    samples = {
        name: np.asarray(value, dtype=np.float64)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    patient_intercept_effect = (
        samples["patient_intercept_sd"][..., None] * samples["patient_intercept_z"]
    )
    patient_slope_effect = samples["patient_slope_sd"][..., None] * samples["patient_slope_z"]
    slide_effect = samples["slide_sd"][..., None] * samples["slide_z"]
    eta = (
        samples["alpha"][..., None]
        + samples["beta"][..., None] * within
        + patient_intercept_effect[..., patient_index]
        + patient_slope_effect[..., patient_index] * within
        + slide_effect[..., slide_index]
    )
    with np.errstate(over="ignore", invalid="ignore"):
        expected_rate = np.exp(eta)
    expected_count = expected_rate * areas
    dispersion = np.exp(samples["log_dispersion"])
    zero_probability = np.exp(
        -dispersion[..., None] * np.log1p(expected_count / dispersion[..., None])
    )
    monitored_names = [
        "alpha", "beta", "log_dispersion", "patient_intercept_sd",
        "patient_slope_sd", "slide_sd", "patient_intercept_z", "patient_slope_z", "slide_z",
    ]
    posterior = az.from_dict(
        {"posterior": {name: samples[name] for name in monitored_names}}
    )
    r_hat = float(
        _flattened(az.rhat(posterior, var_names=monitored_names, method="rank"), monitored_names).max()
    )
    ess_bulk = float(
        _flattened(az.ess(posterior, var_names=monitored_names, method="bulk"), monitored_names).min()
    )
    ess_tail = float(
        _flattened(az.ess(posterior, var_names=monitored_names, method="tail"), monitored_names).min()
    )
    mcse_mean = float(
        _flattened(az.mcse(posterior, var_names=monitored_names, method="mean"), monitored_names).max()
    )
    mcse_sd = float(
        _flattened(az.mcse(posterior, var_names=monitored_names, method="sd"), monitored_names).max()
    )
    extra = sampler.get_extra_fields(group_by_chain=True)
    energy = np.asarray(extra["energy"], dtype=np.float64)
    energy_variance = np.var(energy, axis=1)
    ebfmi_per_chain = np.divide(
        np.mean(np.diff(energy, axis=1) ** 2, axis=1),
        energy_variance,
        out=np.zeros_like(energy_variance),
        where=energy_variance > 0.0,
    )
    minimum_ebfmi = float(ebfmi_per_chain.min())
    divergences = int(np.asarray(extra["diverging"]).sum())
    maximum_steps = 2 ** config["policy"]["maximum_tree_depth"] - 1
    depth_hits = int((np.asarray(extra["num_steps"]) >= maximum_steps).sum())
    rng = np.random.default_rng(seed_for(sampling["seed"], "posterior_predictive"))
    if not (
        all(np.isfinite(value).all() for value in samples.values())
        and np.isfinite(expected_rate).all()
        and np.isfinite(expected_count).all()
        and np.isfinite(zero_probability).all()
    ):
        raise ContractError("posterior draws or expected-count summaries are nonfinite")
    try:
        replicated = _draw_negative_binomial(rng, expected_count, dispersion[..., None])
    except (OverflowError, ValueError) as error:
        raise ContractError("posterior predictive counts cannot be represented") from error
    posterior_finite = bool(
        np.isfinite(replicated).all()
    )
    constraints = bool(
        np.all(dispersion > 0.0)
        and np.all(samples["patient_intercept_sd"] > 0.0)
        and np.all(samples["patient_slope_sd"] > 0.0)
        and np.all(samples["slide_sd"] > 0.0)
        and np.all(expected_count > 0.0)
        and np.all((zero_probability >= 0.0) & (zero_probability <= 1.0))
    )
    policy = config["policy"]
    complete = bool(
        prior_finite
        and posterior_finite
        and constraints
        and r_hat <= policy["maximum_r_hat"]
        and ess_bulk >= policy["minimum_bulk_ess"]
        and ess_tail >= policy["minimum_tail_ess"]
        and minimum_ebfmi >= policy["minimum_ebfmi"]
        and divergences <= policy["maximum_divergences"]
        and depth_hits <= policy["maximum_tree_depth_hits"]
    )
    patient_rows = []
    for patient, patient_id in enumerate(design["patient_ids"]):
        selected = patient_index == patient
        patient_rows.append(
            {
                "patient_id": patient_id,
                "observation_count": int(selected.sum()),
                "random_intercept": summary(patient_intercept_effect[..., patient]),
                "slope": summary(samples["beta"] + patient_slope_effect[..., patient]),
            }
        )
    slide_parent = {}
    for row in observations:
        slide_parent[row["slide_id"]] = row["patient_id"]
    slide_rows = []
    for slide, slide_id in enumerate(design["slide_ids"]):
        selected = slide_index == slide
        slide_rows.append(
            {
                "slide_id": slide_id,
                "patient_id": slide_parent[slide_id],
                "observation_count": int(selected.sum()),
                "random_intercept": summary(slide_effect[..., slide]),
            }
        )
    roi_rows = []
    for index, row in enumerate(observations):
        roi_rows.append(
            {
                "roi_id": row["roi_id"],
                "patient_id": row["patient_id"],
                "slide_id": row["slide_id"],
                "count": row["count"],
                "area": row["area"],
                "predictor": predictor[index],
                "within_predictor": within[index],
                "expected_count": summary(expected_count[..., index]),
                "expected_rate": summary(expected_rate[..., index]),
                "zero_probability": summary(zero_probability[..., index]),
            }
        )
    predictive_patients = []
    for patient, patient_id in enumerate(design["patient_ids"]):
        selected = patient_index == patient
        observed_rate = float(counts[selected].sum() / areas[selected].sum())
        replicated_rates = replicated[..., selected].sum(axis=-1) / areas[selected].sum()
        variance = expected_count[..., selected] + (
            expected_count[..., selected] ** 2 / dispersion[..., None]
        )
        observed_pearson = np.sum(
            (counts[selected] - expected_count[..., selected]) ** 2 / variance, axis=-1
        )
        replicated_pearson = np.sum(
            (replicated[..., selected] - expected_count[..., selected]) ** 2 / variance,
            axis=-1,
        )
        indicator = replicated_pearson >= observed_pearson
        predictive_patients.append(
            {
                "patient_id": patient_id,
                "observed_mean_rate": observed_rate,
                "replicated_mean_rate": float(replicated_rates.mean()),
                "replicated_mean_rate_sd": float(replicated_rates.std(ddof=1)),
                "observed_pearson_discrepancy": float(observed_pearson.mean()),
                "replicated_pearson_discrepancy_mean": float(replicated_pearson.mean()),
                "probability_replicated_pearson_at_least_observed": float(indicator.mean()),
                "pearson_tail_probability_mcse": _chain_aware_mcse_mean(indicator),
            }
        )
    observed_zero_fraction = float(np.mean(counts == 0))
    replicated_zero_fraction = np.mean(replicated == 0, axis=-1)
    zero_indicator = replicated_zero_fraction >= observed_zero_fraction
    posterior_predictive = {
        "observed_zero_fraction": observed_zero_fraction,
        "replicated_zero_fraction_mean": float(replicated_zero_fraction.mean()),
        "replicated_zero_fraction_sd": float(replicated_zero_fraction.std(ddof=1)),
        "probability_replicated_zero_fraction_at_least_observed": float(zero_indicator.mean()),
        "zero_fraction_tail_probability_mcse": _chain_aware_mcse_mean(zero_indicator),
        "patients": predictive_patients,
    }
    beta_summary = summary(samples["beta"])
    rate_ratio_summary = summary(np.exp(samples["beta"]))
    rate_ratio_summary["interval_lower"] = math.exp(beta_summary["interval_lower"])
    rate_ratio_summary["interval_upper"] = math.exp(beta_summary["interval_upper"])
    result = {
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
        "fit_state": "complete" if complete else "nonconverged",
        "claim_status": (
            "experimental_conditional_within_patient_association"
            if complete
            else "diagnostic_only_nonconverged"
        ),
        "model": {
            "likelihood": "negative_binomial_2",
            "linear_predictor": (
                "log_area_offset_plus_within_patient_slope_and_patient_intercept_slope_"
                "and_nested_slide_intercept"
            ),
            "predictor_centering": "within_patient_then_pooled_within_sd",
            "area_unit": config["area_unit"],
            "predictor_name": config["predictor_name"],
            "predictor_unit": config["predictor_unit"],
            "priors": config["priors"],
        },
        "sampling": {
            "chains": sampling["chains"],
            "tune_per_chain": sampling["tune_per_chain"],
            "draws_per_chain": sampling["draws_per_chain"],
            "completed_draws": sampling["chains"] * sampling["draws_per_chain"],
        },
        "diagnostics": {
            "prior_predictive_finite": prior_finite,
            "posterior_finite": posterior_finite,
            "r_hat": r_hat,
            "ess_bulk": ess_bulk,
            "ess_tail": ess_tail,
            "mcse_mean": mcse_mean,
            "mcse_sd": mcse_sd,
            "minimum_ebfmi": minimum_ebfmi,
            "divergences": divergences,
            "max_tree_depth_hits": depth_hits,
            "constraints_valid": constraints,
            "identifiability_checks_passed": True,
        },
        "design": design,
        "posterior": {
            "alpha": summary(samples["alpha"]),
            "beta": beta_summary,
            "rate_ratio": rate_ratio_summary,
            "dispersion": summary(dispersion),
            "patient_intercept_sd": summary(samples["patient_intercept_sd"]),
            "patient_slope_sd": summary(samples["patient_slope_sd"]),
            "slide_sd": summary(samples["slide_sd"]),
        },
        "patients": patient_rows,
        "slides": slide_rows,
        "rois": roi_rows,
        "prior_predictive": prior_predictive,
        "posterior_predictive": posterior_predictive,
    }
    if return_samples:
        return result, samples
    return result


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
    raw = sys.stdin.buffer.read(MAXIMUM_INPUT_BYTES + 1)
    if not raw or len(raw) > MAXIMUM_INPUT_BYTES:
        raise ContractError("request size is invalid")
    request_sha256 = hashlib.sha256(raw).hexdigest()
    config = validate(json.loads(raw), lock_digest, worker_digest)
    result = fit(config, request_sha256, lock_digest, worker_digest)
    encoded = json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")).encode()
    if len(encoded) + 1 > MAXIMUM_OUTPUT_BYTES:
        raise ContractError("worker output exceeds 1 MiB")
    sys.stdout.buffer.write(encoded + b"\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"Marklab NumPyro negative-binomial hierarchy worker failed: "
            f"{type(error).__name__}: {error}",
            file=sys.stderr,
        )
        raise SystemExit(2) from error
