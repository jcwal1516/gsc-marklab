#!/usr/bin/env python3
"""NumPyro agreement worker for the robust Student-t patient hierarchy."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import jax

jax.config.update("jax_enable_x64", True)

import jax.numpy as jnp
import numpy as np
import numpyro
import numpyro.distributions as dist
from numpyro.infer import NUTS

sys.path.append(str(Path(__file__).resolve().parent))

from marklab_worker_support import student_t_hierarchy as source_contract
from marklab_worker_support.numpyro_nuts import (
    nuts_diagnostics,
    result_header,
    run_chains,
    validate_source_request,
)
from marklab_worker_support.nuts_contract import fit_summary
from marklab_worker_support.posterior import summary
from marklab_worker_support.protocol import (
    MIB,
    derive_seed,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    worker_digests,
    write_result,
)
from marklab_worker_support.student_t_hierarchy import (
    hierarchy_summaries,
    student_t_scale_from_sd,
)

REQUEST_FORMAT = "marklab.numpyro_student_t_hierarchy_worker_request"
RESULT_FORMAT = "marklab.numpyro_student_t_hierarchy_worker_result"


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-numpyro-student-t-v1", seed, purpose)


def model(
    patient_index: jnp.ndarray,
    observations: jnp.ndarray,
    patient_count: int,
    global_mean: float,
    global_sd: float,
    between_sd: float,
    observation_sd: float,
    df_rate: float,
) -> None:
    """Student-t patient hierarchy sampled in the likelihood-scale coordinates.

    Likelihood: ``y_ij ~ StudentT(nu, mu_i, s)``. Patient means are non-centered,
    ``mu_i = mu + tau * z_i`` with ``z_i ~ Normal(0, 1)``. Priors: ``mu ~ Normal(global_mean,
    global_sd)``, ``tau ~ HalfNormal(between_sd)``, ``nu = 2 + e`` with
    ``e ~ Exponential(rate=df_rate)``, and ``sigma_obs ~ HalfNormal(observation_sd)`` on the finite
    observation SD ``sigma_obs = s * sqrt(nu / (nu - 2))``. NUTS moves in ``s``, which the data
    identify separately from ``nu``; the prior on ``sigma_obs`` enters through the
    change-of-variables density.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    population = numpyro.sample("global_mean", dist.Normal(global_mean, global_sd))
    between = numpyro.sample("between_patient_sd", dist.HalfNormal(between_sd))
    patient_z = numpyro.sample(
        "patient_z", dist.Normal(0.0, 1.0).expand([patient_count]).to_event(1)
    )
    patient_mean = numpyro.deterministic("patient_mean", population + between * patient_z)
    df_excess = numpyro.sample("degrees_of_freedom_excess", dist.Exponential(df_rate))
    degrees = numpyro.deterministic("degrees_of_freedom", 2.0 + df_excess)
    scale_per_sd = student_t_scale_from_sd(1.0, degrees)
    observation_scale = numpyro.sample(
        "observation_scale", dist.ImproperUniform(dist.constraints.positive, (), ())
    )
    inferred_observation_sd = numpyro.deterministic(
        "observation_sd", observation_scale / scale_per_sd
    )
    numpyro.factor(
        "observation_sd_prior",
        dist.HalfNormal(observation_sd).log_prob(inferred_observation_sd) - jnp.log(scale_per_sd),
    )
    numpyro.sample(
        "observation",
        dist.StudentT(degrees, patient_mean[patient_index], observation_scale),
        obs=observations,
    )


def observation_patients(config: dict[str, Any]) -> np.ndarray:
    """The patient index of each observation, in concatenated observation order."""
    return np.concatenate(
        [
            np.full(len(values), index, dtype=np.int32)
            for index, values in enumerate(config["patient_values"])
        ]
    )


def model_arguments(config: dict[str, Any], patient_index: np.ndarray, observations: Any) -> dict:
    """Keyword arguments of ``model`` for the given observations."""
    return {
        "patient_index": jnp.asarray(patient_index),
        "observations": jnp.asarray(observations),
        "patient_count": len(config["patient_ids"]),
        "global_mean": config["global_mean"],
        "global_sd": config["global_sd"],
        "between_sd": config["between_sd"],
        "observation_sd": config["observation_sd"],
        "df_rate": config["df_rate"],
    }


def prior_predictive_is_finite(config: dict[str, Any], patient_index: np.ndarray) -> bool:
    """Simulate the prior predictive with NumPy and report whether every draw is finite."""
    rng = np.random.default_rng(seed_for(config["seed"], "prior"))
    draws = config["prior_draws"]
    patient_count = len(config["patient_ids"])
    population = rng.normal(config["global_mean"], config["global_sd"], draws)
    between_patient_sd = np.abs(rng.normal(0.0, config["between_sd"], draws))
    observation_sd = np.abs(rng.normal(0.0, config["observation_sd"], draws))
    degrees_of_freedom = 2.0 + rng.exponential(1.0 / config["df_rate"], draws)
    if not (
        np.isfinite(population).all()
        and np.isfinite(between_patient_sd).all()
        and np.isfinite(observation_sd).all()
        and np.isfinite(degrees_of_freedom).all()
    ):
        return False
    # Only a finiteness flag is needed; keep one generated observation vector
    # at a time instead of retaining prior_draws * observation_count values.
    for mean, between, sd, degrees in zip(
        population, between_patient_sd, observation_sd, degrees_of_freedom, strict=True
    ):
        patient_z = rng.normal(size=patient_count)
        with np.errstate(over="ignore", invalid="ignore"):
            patient_mean = mean + between * patient_z
            observation_scale = student_t_scale_from_sd(sd, degrees)
            observations = patient_mean[patient_index] + observation_scale * rng.standard_t(
                degrees, size=patient_index.size
            )
        if not (
            np.isfinite(patient_z).all()
            and np.isfinite(patient_mean).all()
            and np.isfinite(observation_scale)
            and np.isfinite(observations).all()
        ):
            return False
    return True


def fit(
    config: dict[str, Any], request_sha: str, source_sha: str, lock_sha: str, worker_sha: str
) -> dict[str, Any]:
    patient_index = observation_patients(config)
    observations = np.concatenate(config["patient_values"])
    kernel = NUTS(
        model,
        target_accept_prob=config["target_accept"],
        max_tree_depth=config["maximum_tree_depth"],
        dense_mass=True,
    )
    sampler, samples = run_chains(
        kernel,
        config,
        jax.random.PRNGKey(seed_for(config["seed"], "nuts")),
        model_arguments(config, patient_index, observations),
    )
    degrees = 2.0 + samples["degrees_of_freedom_excess"]
    monitored = {
        "global_mean": samples["global_mean"],
        "between_patient_sd": samples["between_patient_sd"],
        "patient_mean": samples["patient_mean"],
        "observation_sd": samples["observation_sd"],
        "degrees_of_freedom": degrees,
    }
    diagnostics = nuts_diagnostics(
        monitored,
        list(monitored),
        sampler.get_extra_fields(group_by_chain=True),
        config["maximum_tree_depth"],
        mcse=True,
    )
    # Posterior predictive observations, simulated with NumPy from the posterior draws.
    rng = np.random.default_rng(seed_for(config["seed"], "predictive"))
    patient_draws = samples["patient_mean"]
    predictive_scale = student_t_scale_from_sd(samples["observation_sd"], degrees)
    predictive_location = patient_draws[..., patient_index]
    predictive = predictive_location + predictive_scale[..., None] * rng.standard_t(
        degrees[..., None], size=predictive_location.shape
    )
    prior_finite = prior_predictive_is_finite(config, patient_index)
    posterior_finite = bool(
        all(np.isfinite(value).all() for value in monitored.values())
        and np.isfinite(predictive).all()
    )
    constraints = bool(
        np.all(samples["between_patient_sd"] > 0.0)
        and np.all(samples["observation_sd"] > 0.0)
        and np.all(degrees > 2.0)
    )
    return {
        **result_header(RESULT_FORMAT, lock_sha, worker_sha, request_sha, source_sha),
        **fit_summary(diagnostics, config, prior_finite, posterior_finite, constraints),
        "posterior": {
            "global_mean": summary(samples["global_mean"], chains=config["chains"]),
            "between_patient_sd": summary(samples["between_patient_sd"], chains=config["chains"]),
            "observation_sd": summary(samples["observation_sd"], chains=config["chains"]),
            "degrees_of_freedom": summary(degrees, chains=config["chains"]),
        },
        **hierarchy_summaries(
            config,
            observations,
            patient_index,
            samples["global_mean"],
            patient_draws.reshape(-1, patient_draws.shape[-1]),
            predictive.reshape(-1, observations.size),
        ),
    }


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(2 * MIB)
    _, config, source_sha = validate_source_request(
        json.loads(raw), REQUEST_FORMAT, lock_sha, worker_sha, source_contract
    )
    result = fit(config, sha256_hex(raw), source_sha, lock_sha, worker_sha)
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "Marklab NumPyro Student-t worker failed")
