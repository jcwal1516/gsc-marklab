"""NumPyro model pieces shared by the replicated multitype LGCP workers.

Importing workers enable ``jax_enable_x64`` before importing this module.
"""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

import jax
import jax.numpy as jnp
import numpy as np
import numpyro
import numpyro.distributions as dist
from numpyro.infer import Predictive

from .numpyro_nuts import sample_nuts
from .nuts_contract import convergence_passes
from .protocol import backend_identity, derive_seed

FIXED_KERNEL_SEED_DOMAIN = "marklab-numpyro-replicated-multitype-lgcp-v1"
"""Seed domain of the fixed-kernel NumPyro worker; it also seeds every prior count check."""


def hierarchical_model(
    config: dict[str, Any],
    kernel_priors: Callable[[], Any],
    latent_field: Callable[[Any, Any], Any],
) -> Callable[[], None]:
    """Return the NumPyro replicated multitype LGCP (see ``multitype_lgcp``).

    Per type: ``intercept ~ Normal(intercept_mean, intercept_sd)``, ``group_effect ~
    Normal(0, group_effect_sd)``, ``covariate_effect ~ Normal(0, covariate_effect_sd)``,
    ``patient_sd ~ HalfNormal(patient_sd_scale)``, ``pattern_sd ~ HalfNormal(pattern_sd_scale)``.
    ``kernel_priors()`` then samples the latent-field kernel parameters. Patient, pattern, and
    field effects are non-centred: standard-normal ``patient_raw``, ``pattern_raw`` (centred
    within each patient), and ``field_raw`` scaled by their SDs. ``latent_field(field_raw,
    kernel)`` returns the ``(type, node)`` latent field, centred within each pattern. Counts are
    Poisson with the quadrature-weighted intensity.
    """
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

    def type_vector(name: str, distribution: Any) -> Any:
        return numpyro.sample(name, distribution.expand([types]).to_event(1))

    def standard_normal(name: str, shape: list[int]) -> Any:
        return numpyro.sample(name, dist.Normal(0.0, 1.0).expand(shape).to_event(2))

    def model() -> None:
        intercept = type_vector(
            "intercept", dist.Normal(priors["intercept_mean"], priors["intercept_sd"])
        )
        group_effect = type_vector("group_effect", dist.Normal(0.0, priors["group_effect_sd"]))
        covariate_effect = type_vector(
            "covariate_effect", dist.Normal(0.0, priors["covariate_effect_sd"])
        )
        patient_sd = type_vector("patient_sd", dist.HalfNormal(priors["patient_sd_scale"]))
        pattern_sd = type_vector("pattern_sd", dist.HalfNormal(priors["pattern_sd_scale"]))
        kernel = kernel_priors()
        patient_raw = standard_normal("patient_raw", [patients, types])
        pattern_raw = standard_normal("pattern_raw", [patterns, types])
        field_raw = standard_normal("field_raw", [types, nodes])
        patient_effect = numpyro.deterministic("patient_effect", patient_sd[None, :] * patient_raw)
        pattern_sums = jnp.zeros((patients, types)).at[pattern_patients].add(pattern_raw)
        pattern_numbers = jnp.zeros(patients).at[pattern_patients].add(1.0)
        patient_pattern_mean = (
            pattern_sums[pattern_patients] / pattern_numbers[pattern_patients, None]
        )
        pattern_effect = numpyro.deterministic(
            "pattern_effect", pattern_sd[None, :] * (pattern_raw - patient_pattern_mean)
        )
        latent = numpyro.deterministic("latent_effect", latent_field(field_raw, kernel))
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
        expected = numpyro.deterministic("expected_count", expected_matrix[type_index, node_index])
        numpyro.sample("observed_count", dist.Poisson(expected), obs=observed)

    return model


def prior_is_finite(model: Callable[[], None], draws: int, seed: int) -> bool:
    """Whether prior-predictive rates are finite and positive and allow Poisson count draws.

    Rates come from the model's own priors with the observations held fixed; conditional counts
    are then drawn with the NumPy Poisson sampler used for the posterior predictive, because
    finite rates can still exceed the sampler's integer range.
    """
    predictive = Predictive(model, num_samples=draws, return_sites=["expected_count"])
    expected = np.asarray(predictive(jax.random.PRNGKey(seed))["expected_count"])
    if not np.isfinite(expected).all() or not np.all(expected > 0.0):
        return False
    count_seed = derive_seed(FIXED_KERNEL_SEED_DOMAIN, seed, "prior_counts")
    try:
        counts = np.random.default_rng(count_seed).poisson(expected)
    except ValueError:
        return False
    return bool(np.isfinite(counts).all() and np.all(counts >= 0))


def sample_posterior(
    model: Callable[[], None], config: dict[str, Any], seed: int
) -> tuple[dict[str, np.ndarray], dict[str, np.ndarray], dict[str, Any]]:
    """Run the shared diagonal-mass NUTS policy and return grouped, flat, and extra draws."""
    samples, extra = sample_nuts(model, config, config["numpyro_maximum_tree_depth"], seed)
    flat = {name: value.reshape((-1, *value.shape[2:])) for name, value in samples.items()}
    return samples, flat, extra


def fit_disposition(
    config: dict[str, Any],
    diagnostics: dict[str, Any],
    constraints: bool,
    values: tuple[np.ndarray, ...],
) -> tuple[bool, bool]:
    """Return posterior finiteness and the common convergence/constraint disposition."""
    finite = bool(all(np.isfinite(value).all() for value in values))
    complete = bool(finite and constraints and convergence_passes(config["policy"], diagnostics))
    return finite, complete


def result_header(
    result_format: str,
    config: dict[str, Any],
    request_sha: str,
    source_sha: str,
    lock_sha: str,
    worker_sha: str,
    complete: bool,
    *,
    report_maximum_tree_depth: bool,
) -> dict[str, Any]:
    """Common identity, disposition, and sampling fields for multitype LGCP results."""
    result = {
        "format": result_format,
        "version": 1,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_sha, worker_sha),
        "jax_version": jax.__version__,
        "input_sha256": config["input_sha"],
        "request_sha256": request_sha,
        "source_request_sha256": source_sha,
    }
    if report_maximum_tree_depth:
        result["maximum_tree_depth"] = config["numpyro_maximum_tree_depth"]
    result.update(
        {
            "fit_state": "complete" if complete else "nonconverged",
            "sampling": {
                "chains": config["chains"],
                "tune_per_chain": config["tune"],
                "draws_per_chain": config["draws"],
                "completed_draws": config["chains"] * config["draws"],
            },
        }
    )
    return result


def diagnostic_report(
    prior_finite: bool,
    posterior_finite: bool,
    diagnostics: dict[str, Any],
    constraints: bool,
) -> dict[str, Any]:
    """Common finite, convergence, and identifiability diagnostic fields."""
    return {
        "prior_predictive_finite": prior_finite,
        "posterior_finite": posterior_finite,
        **diagnostics,
        "constraints_valid": constraints,
        "identifiability_checks_passed": constraints,
    }
