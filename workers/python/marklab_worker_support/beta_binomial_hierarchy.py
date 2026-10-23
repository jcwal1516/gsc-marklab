"""Backend-neutral request contract for the patient beta-binomial hierarchy."""

from __future__ import annotations

from typing import Any

import numpy as np

from . import pymc_request
from .protocol import ContractError, exact, integer, number, obj

SOURCE_WORKER = "marklab_pymc_beta_binomial_hierarchy_worker.py"

MODEL_KEYS = {
    "format",
    "version",
    "family",
    "population_probability_prior",
    "population_alpha",
    "population_beta",
    "concentration_prior",
    "concentration_prior_sd",
    "patient_probability",
    "likelihood",
    "overdispersion",
    "biological_unit",
    "backend_capability",
    "maturity",
}


def validate_request(request: Any, lock_digest: str, worker_digest: str) -> dict[str, Any]:
    request, model = pymc_request.validate_envelope(
        request, lock_digest, worker_digest, ("patients",), MODEL_KEYS
    )
    exact(model["family"], "patient_beta_binomial_hierarchy", "model.family")
    exact(model["population_probability_prior"], "beta", "model.population prior")
    exact(model["concentration_prior"], "half_normal", "model.concentration prior")
    exact(
        model["patient_probability"],
        "beta_population_mean_concentration",
        "model.patient probability",
    )
    exact(model["likelihood"], "binomial_successes_given_patient_trials", "model.likelihood")
    population_alpha = number(model["population_alpha"], "model.population_alpha")
    population_beta = number(model["population_beta"], "model.population_beta")
    concentration_sd = number(model["concentration_prior_sd"], "model.concentration_sd")
    if min(population_alpha, population_beta, concentration_sd) <= 0.0:
        raise ContractError("model priors must be positive")
    patients = request["patients"]
    if not isinstance(patients, list) or not 3 <= len(patients) <= 512:
        raise ContractError("patient count is invalid")
    patient_ids: list[str] = []
    successes: list[int] = []
    trials: list[int] = []
    for index, raw in enumerate(patients):
        row = obj(raw, {"patient_id", "successes", "trials"}, f"patients[{index}]")
        patient_id = row["patient_id"]
        if not isinstance(patient_id, str) or not patient_id or len(patient_id) > 128:
            raise ContractError("patient identity is invalid")
        patient_ids.append(patient_id)
        trial = integer(row["trials"], "patient.trials", 1, 10_000_000)
        successes.append(integer(row["successes"], "patient.successes", 0, trial))
        trials.append(trial)
    if patient_ids != sorted(patient_ids) or len(set(patient_ids)) != len(patient_ids):
        raise ContractError("patient identities must be unique and sorted")
    sampling = pymc_request.validate_sampling(request["sampling"])
    resources = obj(
        request["resources"],
        {
            "maximum_patients",
            "maximum_total_trials",
            "maximum_total_iterations",
            "maximum_output_bytes",
            "timeout_seconds",
        },
        "resources",
    )
    if len(patients) > integer(resources["maximum_patients"], "resources.patients", 3, 512):
        raise ContractError("patient limit exceeded")
    if sum(trials) > integer(resources["maximum_total_trials"], "resources.trials", 1, 10_000_000):
        raise ContractError("trial limit exceeded")
    iterations = sampling["chains"] * (sampling["tune"] + sampling["draws"])
    if iterations > integer(
        resources["maximum_total_iterations"], "resources.iterations", 1, 400_000
    ):
        raise ContractError("iteration limit exceeded")
    policy = pymc_request.validate_policy(request["diagnostic_policy"])
    return {
        "population_alpha": population_alpha,
        "population_beta": population_beta,
        "concentration_sd": concentration_sd,
        "patient_ids": patient_ids,
        "successes": np.asarray(successes, dtype=np.int64),
        "trials": np.asarray(trials, dtype=np.int64),
        **sampling,
        **policy,
    }
