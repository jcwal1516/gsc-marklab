#!/usr/bin/env python3
"""NumPyro agreement worker for the patient Dirichlet-multinomial two-group model."""

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

sys.path.append(str(Path(__file__).resolve().parent))

import marklab_pymc_dirichlet_multinomial_group_worker as pymc_contract
from marklab_worker_support.count_summaries import (
    class_posterior_report,
    class_probabilities,
    predictive_class_report,
)
from marklab_worker_support.numpyro_nuts import (
    fit_with_predictive,
    nuts_diagnostics,
    posterior_is_finite,
    prior_is_finite,
    result_header,
    validate_source_request,
)
from marklab_worker_support.nuts_contract import fit_summary
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

REQUEST_FORMAT = "marklab.numpyro_dirichlet_multinomial_group_worker_request"
RESULT_FORMAT = "marklab.numpyro_dirichlet_multinomial_group_worker_result"
MONITORED = ["baseline_logits", "group_log_ratio_effects", "concentration"]


def seed_for(seed: int, purpose: str) -> int:
    return derive_seed("marklab-numpyro-dirichlet-multinomial-group-v1", seed, purpose)


def model(
    observed_counts: jnp.ndarray | None,
    totals: jnp.ndarray,
    total_count_max: int,
    indicator: jnp.ndarray,
    classes: int,
    logit_sd: float,
    group_sd: float,
    concentration_sd: float,
) -> None:
    """Collapsed patient Dirichlet-multinomial two-group model.

    Likelihood: class-count vectors ``y_i ~ DirichletMultinomial(N_i, phi * pi_g(i))``. Class
    probabilities are ``pi_ref = softmax([b, 0])`` and ``pi_cmp = softmax([b + d, 0])`` with
    the last class as the zero-logit reference. Priors: baseline logits
    ``b_k ~ Normal(0, logit_sd)``, group log-ratio effects ``d_k ~ Normal(0, group_sd)``, and
    concentration ``phi ~ HalfNormal(concentration_sd)``.
    Normal scales are SDs; half-normal scales are the SDs of their underlying normal distributions.
    """
    baseline = numpyro.sample("baseline_logits", dist.Normal(0.0, logit_sd).expand((classes - 1,)))
    effect = numpyro.sample(
        "group_log_ratio_effects", dist.Normal(0.0, group_sd).expand((classes - 1,))
    )
    concentration = numpyro.sample("concentration", dist.HalfNormal(concentration_sd))
    zero = jnp.zeros((1,))
    reference_probability = jax.nn.softmax(jnp.concatenate((baseline, zero)))
    comparison_probability = jax.nn.softmax(jnp.concatenate((baseline + effect, zero)))
    group_probability = jnp.stack((reference_probability, comparison_probability))
    patient_probability = group_probability[indicator]
    numpyro.sample(
        "counts",
        dist.DirichletMultinomial(
            concentration=patient_probability * concentration,
            total_count=totals,
            total_count_max=total_count_max,
        ),
        obs=observed_counts,
    )


def fit(
    config: dict[str, Any], request_sha: str, source_sha: str, lock_sha: str, worker_sha: str
) -> dict[str, Any]:
    arguments = {
        "totals": jnp.asarray(config["totals"]),
        "total_count_max": int(np.max(config["totals"])),
        "indicator": jnp.asarray(config["indicator"]),
        "classes": len(config["class_ids"]),
        "logit_sd": config["logit_sd"],
        "group_sd": config["group_sd"],
        "concentration_sd": config["concentration_sd"],
    }
    prior, sampler, samples, replicated = fit_with_predictive(
        model,
        config,
        seed_for,
        "observed_counts",
        jnp.asarray(config["counts"]),
        arguments,
        "counts",
    )
    extra = sampler.get_extra_fields(group_by_chain=True)
    diagnostics = nuts_diagnostics(
        samples, MONITORED, extra, config["maximum_tree_depth"], mcse=True
    )
    reference, comparison = class_probabilities(
        samples["baseline_logits"], samples["group_log_ratio_effects"]
    )
    posterior, constraints = class_posterior_report(
        config["class_ids"], reference, comparison, samples["concentration"], config["chains"]
    )
    return {
        **result_header(RESULT_FORMAT, lock_sha, worker_sha, request_sha, source_sha),
        **fit_summary(
            diagnostics,
            config,
            prior_is_finite(prior),
            posterior_is_finite(samples, replicated),
            constraints,
        ),
        "posterior": posterior,
        "posterior_predictive": {"classes": predictive_class_report(config, replicated)},
    }


def main() -> None:
    require_versions(numpyro=numpyro.__version__, jax=jax.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(2 * MIB)
    _, config, source_sha = validate_source_request(
        json.loads(raw), REQUEST_FORMAT, lock_sha, worker_sha, pymc_contract
    )
    result = fit(config, sha256_hex(raw), source_sha, lock_sha, worker_sha)
    write_result(result, max_bytes=2 * MIB)


if __name__ == "__main__":
    run_worker(main, "Marklab NumPyro Dirichlet-multinomial group worker failed")
