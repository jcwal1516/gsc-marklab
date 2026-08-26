#!/usr/bin/env python3
"""Static audited PyMC annealed-SMC worker for the conjugate normal mean."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any

import numpy as np
import pymc as pm

PYMC_VERSION = "6.3.0"


class ContractError(ValueError):
    pass


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
        {"format", "version", "backend", "model", "observations", "sampling", "resources"},
        "request",
    )
    exact(request["format"], "marklab.pymc_worker_request", "request.format")
    integer(request["version"], "request.version", 1, 1)
    backend = obj(
        request["backend"],
        {"name", "version", "python_version", "environment_lock_sha256", "worker_sha256"},
        "backend",
    )
    exact(backend["name"], "pymc", "backend.name")
    exact(backend["version"], PYMC_VERSION, "backend.version")
    exact(backend["python_version"], "3.12", "backend.python")
    exact(backend["environment_lock_sha256"], lock_digest, "backend.lock")
    exact(backend["worker_sha256"], worker_digest, "backend.worker")
    model = obj(
        request["model"],
        {
            "format",
            "version",
            "family",
            "parameter",
            "prior",
            "likelihood",
            "observation_unit",
            "generated_quantities",
            "backend_capability",
            "maturity",
        },
        "model",
    )
    exact(model["format"], "marklab.bayesian_model_ir", "model.format")
    integer(model["version"], "model.version", 1, 1)
    exact(model["family"], "normal_mean_known_sigma", "model.family")
    parameter = obj(model["parameter"], {"name", "support", "interpretation"}, "parameter")
    exact(parameter["name"], "mu", "parameter.name")
    exact(parameter["support"], "real", "parameter.support")
    exact(parameter["interpretation"], "population_mean", "parameter.interpretation")
    prior = obj(model["prior"], {"family", "mean", "sd", "rationale"}, "prior")
    exact(prior["family"], "normal", "prior.family")
    prior_mean = number(prior["mean"], "prior.mean")
    prior_sd = number(prior["sd"], "prior.sd")
    exact(prior["rationale"], "user_supplied", "prior.rationale")
    likelihood = obj(model["likelihood"], {"family", "known_sigma"}, "likelihood")
    exact(likelihood["family"], "normal_known_sigma", "likelihood.family")
    known_sigma = number(likelihood["known_sigma"], "likelihood.sigma")
    if prior_sd <= 0.0 or known_sigma <= 0.0:
        raise ContractError("normal scales must be positive")
    exact(model["observation_unit"], "scalar_observation", "model.observation_unit")
    exact(
        model["generated_quantities"],
        ["posterior_predictive_observation_mean"],
        "model.generated",
    )
    exact(model["backend_capability"], "annealed_smc", "model.capability")
    exact(model["maturity"], "experimental", "model.maturity")
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
    if len(observations) > integer(
        resources["maximum_observations"], "resources.observations", 1, 100_000
    ):
        raise ContractError("observation count exceeds resource limit")
    max_particles = integer(
        resources["maximum_total_particles"], "resources.particles", 1, 40_000
    )
    integer(resources["maximum_output_bytes"], "resources.output", 1, 16 * 1024 * 1024)
    integer(resources["timeout_seconds"], "resources.timeout", 1, 3_600)
    if particles * chains > max_particles:
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
    digest = hashlib.sha256(f"marklab-pymc-smc-v1\0{seed}\0{purpose}\0{index}".encode()).digest()
    return int.from_bytes(digest[:4], "little")


def scalar_summary(draws: np.ndarray) -> dict[str, float]:
    return {
        "mean": float(draws.mean()),
        "sd": float(draws.std(ddof=1)),
        "interval_lower": float(np.quantile(draws, 0.025)),
        "interval_upper": float(np.quantile(draws, 0.975)),
    }


def fit(config: dict[str, Any], request_sha: str, lock_sha: str, worker_sha: str) -> dict[str, Any]:
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
            ancestry_valid = ancestry_valid and len(ancestors) == config["particles"] and all(
                0 <= value < config["particles"] for value in ancestors
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
            float(value)
            for value in evidence_values[chain]
            if math.isfinite(float(value))
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
        "backend": {
            "name": "pymc",
            "version": pm.__version__,
            "python_version": f"{sys.version_info.major}.{sys.version_info.minor}",
            "environment_lock_sha256": lock_sha,
            "worker_sha256": worker_sha,
        },
        "request_sha256": request_sha,
        "fit_state": "complete" if complete else "nonconverged",
        "posterior": scalar_summary(flat_particles),
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
    if pm.__version__ != PYMC_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("PyMC or Python version drift")
    script = Path(__file__)
    lock_sha = hashlib.sha256(script.with_name("uv.lock").read_bytes()).hexdigest()
    worker_sha = hashlib.sha256(script.read_bytes()).hexdigest()
    raw = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(raw) > 16 * 1024 * 1024:
        raise ContractError("request exceeds 16 MiB")
    request_sha = hashlib.sha256(raw).hexdigest()
    result = fit(validate(json.loads(raw), lock_sha, worker_sha), request_sha, lock_sha, worker_sha)
    sys.stdout.write(json.dumps(result, allow_nan=False, sort_keys=True, separators=(",", ":")))
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"marklab PyMC worker failed: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from error
