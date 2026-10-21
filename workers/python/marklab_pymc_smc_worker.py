#!/usr/bin/env python3
"""PyMC annealed-SMC worker for the conjugate normal mean, with an audited IMH kernel."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any

sys.path.append(str(Path(__file__).resolve().parent))

# Configure PyTensor before PyMC is imported.
from marklab_worker_support import pytensor_config  # noqa: F401

# isort: split
import numpy as np
import pymc as pm

from marklab_worker_support import versions
from marklab_worker_support.normal_models import PYMC_REQUEST_FORMAT, normal_mean_model
from marklab_worker_support.posterior import summary
from marklab_worker_support.protocol import (
    MIB,
    ContractError,
    backend_identity,
    derive_seed,
    exact,
    integer,
    number,
    obj,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    validate_backend,
    worker_digests,
    write_result,
)


def validate(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request = obj(
        request,
        {"format", "version", "backend", "model", "observations", "sampling", "resources"},
        "request",
    )
    exact(request["format"], PYMC_REQUEST_FORMAT, "request.format")
    integer(request["version"], "request.version", 1, 1)
    validate_backend(
        request["backend"],
        "backend",
        name="pymc",
        version=versions.PYMC,
        lock_sha256=lock_digest,
        worker_sha256=worker_digest,
    )
    prior_mean, prior_sd, known_sigma = normal_mean_model(request["model"], "model", "annealed_smc")
    observations_raw = request["observations"]
    if not isinstance(observations_raw, list) or not 1 <= len(observations_raw) <= 100_000:
        raise ContractError("observations must contain 1-100000 values")
    observations = np.asarray(
        [number(value, "observations[]") for value in observations_raw], dtype=np.float64
    )
    sampling = obj(
        request["sampling"],
        {"particles", "chains", "ess_target", "correlation_threshold", "seed"},
        "sampling",
    )
    particles = integer(sampling["particles"], "sampling.particles", 100, 10_000)
    chains = integer(sampling["chains"], "sampling.chains", 2, 4)
    ess_target = number(sampling["ess_target"], "sampling.ess_target")
    correlation_threshold = number(
        sampling["correlation_threshold"], "sampling.correlation_threshold"
    )
    if not 0.0 < ess_target < 1.0 or not 0.0 < correlation_threshold < 1.0:
        raise ContractError("SMC thresholds must be in (0,1)")
    seed = integer(sampling["seed"], "sampling.seed", 0, 2**64 - 1)
    resources = obj(
        request["resources"],
        {
            "maximum_observations",
            "maximum_total_particles",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    maximum_observations = integer(
        resources["maximum_observations"], "resources.maximum_observations", 1, 100_000
    )
    if len(observations) > maximum_observations:
        raise ContractError("observation count exceeds resource limit")
    maximum_particles = integer(
        resources["maximum_total_particles"], "resources.maximum_total_particles", 1, 40_000
    )
    maximum_output_bytes = integer(
        resources["maximum_output_bytes"], "resources.maximum_output_bytes", 1, 16 * MIB
    )
    integer(resources["timeout_seconds"], "resources.timeout_seconds", 1, 3_600)
    if particles * chains > maximum_particles:
        raise ContractError("total particles exceed resource limit")
    return {
        "prior_mean": prior_mean,
        "prior_sd": prior_sd,
        "known_sigma": known_sigma,
        "observations": observations,
        "particles": particles,
        "chains": chains,
        "ess_target": ess_target,
        "correlation_threshold": correlation_threshold,
        "seed": seed,
        "maximum_output_bytes": maximum_output_bytes,
    }


class AuditedIMH(pm.smc.kernels.IMH):
    """Pinned IMH kernel that exposes each stage's ESS and exact ancestry."""

    def __init__(self, *args: Any, **kwargs: Any):
        super().__init__(*args, **kwargs)
        self.stats_dtypes_shapes = {
            **super().stats_dtypes_shapes,
            "ess": (float, []),
            "ancestor_indexes": (int, [self.draws]),
            "prior_particles_finite": (bool, []),
        }
        self.ess = float(self.draws)
        self.ancestor_indexes = np.arange(self.draws, dtype=np.int64)
        self.prior_particles_finite = False

    def setup_kernel(self) -> None:
        self.prior_particles_finite = bool(
            np.isfinite(self.tempered_posterior).all()
            and np.isfinite(self.prior_logp).all()
            and np.isfinite(self.likelihood_logp).all()
        )

    def update_beta_and_weights(self) -> None:
        super().update_beta_and_weights()
        self.ess = float(1.0 / np.square(self.weights).sum())

    def resample(self) -> None:
        super().resample()
        self.ancestor_indexes = np.asarray(self.resampling_indexes, dtype=np.int64).copy()

    def sample_stats(self) -> dict[str, Any]:
        return {
            **super().sample_stats(),
            "ess": self.ess,
            "ancestor_indexes": self.ancestor_indexes,
            "prior_particles_finite": self.prior_particles_finite,
        }


def seed_for(seed: int, purpose: str, index: int = 0) -> int:
    return derive_seed("marklab-pymc-smc-v1", seed, purpose, index)


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
    """Sample the normal-mean posterior by sequential Monte Carlo with an independent
    Metropolis-Hastings kernel.

    Model: ``y_i ~ Normal(mu, known_sigma)`` with ``mu ~ Normal(prior_mean, prior_sd)``. Each
    chain tempers from the prior to the posterior; the kernel records per-stage ESS, ancestor
    indexes, and prior-particle finiteness for the reported stage diagnostics.
    """
    with pm.Model():
        mu = pm.Normal("mu", config["prior_mean"], config["prior_sd"])
        pm.Normal("observed", mu, config["known_sigma"], observed=config["observations"])
        posterior = pm.sample_smc(
            draws=config["particles"],
            kernel=AuditedIMH,
            chains=config["chains"],
            cores=1,
            blas_cores=1,
            random_seed=[seed_for(config["seed"], "chain", i) for i in range(config["chains"])],
            threshold=config["ess_target"],
            correlation_threshold=config["correlation_threshold"],
            progressbar=False,
            compute_convergence_checks=False,
        )
    particles = np.asarray(posterior["posterior"]["mu"].values, dtype=np.float64)
    stats = posterior["sample_stats"].dataset
    beta_values = np.asarray(stats["beta"].values, dtype=object)
    ess_values = np.asarray(stats["ess"].values, dtype=object)
    acceptance_values = np.asarray(stats["accept_rate"].values, dtype=object)
    ancestor_values = np.asarray(stats["ancestor_indexes"].values, dtype=object)
    evidence_values = np.asarray(stats["log_marginal_likelihood"].values, dtype=object)
    prior_values = np.asarray(stats["prior_particles_finite"].values, dtype=object)
    chains = []
    all_ess: list[float] = []
    all_acceptance: list[float] = []
    chain_evidence: list[float] = []
    ancestry_valid = True
    prior_finite = True
    final_beta_one = True
    for chain in range(config["chains"]):
        stages = []
        prior_beta = 0.0
        for stage_index in range(beta_values.shape[1]):
            beta = float(beta_values[chain, stage_index])
            if not math.isfinite(beta):
                continue
            ess = float(ess_values[chain, stage_index])
            acceptance = float(acceptance_values[chain, stage_index])
            ancestors = [int(value) for value in ancestor_values[chain, stage_index]]
            ancestry_valid = (
                ancestry_valid
                and len(ancestors) == config["particles"]
                and all(0 <= value < config["particles"] for value in ancestors)
            )
            prior_finite = prior_finite and bool(prior_values[chain, stage_index])
            if not beta > prior_beta:
                raise ContractError("SMC beta path is not strictly increasing")
            prior_beta = beta
            all_ess.append(ess)
            all_acceptance.append(acceptance)
            stages.append(
                {
                    "beta": beta,
                    "ess": ess,
                    "acceptance_rate": acceptance,
                    "ancestor_indexes": ancestors,
                }
            )
        final_beta_one = final_beta_one and bool(stages) and stages[-1]["beta"] == 1.0
        evidence_candidates = [
            float(value) for value in evidence_values[chain] if math.isfinite(float(value))
        ]
        if len(evidence_candidates) != 1:
            raise ContractError("SMC chain lacks one final evidence estimate")
        evidence = evidence_candidates[0]
        chain_evidence.append(evidence)
        chains.append(
            {
                "chain": chain,
                "log_marginal_likelihood": evidence,
                "stages": stages,
            }
        )
    flat_particles = particles.reshape(-1)
    rng = np.random.default_rng(seed_for(config["seed"], "predictive"))
    predictive = rng.normal(
        flat_particles[:, None],
        config["known_sigma"],
        size=(flat_particles.size, config["observations"].size),
    )
    replicated_means = predictive.mean(axis=1)
    observed_mean = float(config["observations"].mean())
    evidence_array = np.asarray(chain_evidence)
    posterior_finite = bool(np.isfinite(flat_particles).all())
    predictive_finite = bool(np.isfinite(predictive).all())
    evidence_finite = bool(np.isfinite(evidence_array).all())
    complete = (
        prior_finite
        and posterior_finite
        and predictive_finite
        and final_beta_one
        and ancestry_valid
        and evidence_finite
    )
    return {
        "format": "marklab.pymc_normal_mean_smc_worker_result",
        "version": 1,
        "backend": backend_identity("pymc", pm.__version__, lock_sha, worker_sha),
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "posterior": summary(flat_particles),
        "evidence": {
            "chain_log_marginal_likelihoods": chain_evidence,
            "mean": float(evidence_array.mean()),
            "sd": float(evidence_array.std(ddof=1)),
        },
        "chains": chains,
        "diagnostics": {
            "prior_particles_finite": prior_finite,
            "posterior_particles_finite": posterior_finite,
            "posterior_predictive_finite": predictive_finite,
            "all_final_beta_one": final_beta_one,
            "ancestry_valid": ancestry_valid,
            "evidence_finite": evidence_finite,
            "minimum_ess": float(min(all_ess)),
            "minimum_ess_ratio": float(min(all_ess) / config["particles"]),
            "minimum_acceptance_rate": float(min(all_acceptance)),
            "maximum_acceptance_rate": float(max(all_acceptance)),
        },
        "posterior_predictive": {
            "observed_mean": observed_mean,
            "replicated_mean_mean": float(replicated_means.mean()),
            "replicated_mean_sd": float(replicated_means.std(ddof=1)),
            "probability_replicated_mean_at_least_observed": float(
                np.mean(replicated_means >= observed_mean)
            ),
        },
    }


def main() -> None:
    require_versions(pymc=pm.__version__)
    lock_sha, worker_sha = worker_digests(__file__)
    raw = read_request(16 * MIB)
    config = validate(json.loads(raw), lock_sha, worker_sha)
    result = fit(config, sha256_hex(raw), lock_sha, worker_sha)
    write_result(result, max_bytes=config["maximum_output_bytes"])


if __name__ == "__main__":
    run_worker(main, "marklab PyMC worker failed")
