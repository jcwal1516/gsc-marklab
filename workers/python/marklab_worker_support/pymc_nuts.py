"""PyMC sampling shared by the NUTS workers.

Every PyMC NUTS worker samples the prior predictive, runs PyMC's own NUTS sampler one seeded
chain at a time on one core, and draws the posterior predictive, all on the model on the context
stack. The request blocks that configure these calls are validated by ``nuts_contract``.
"""

from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import Any

from . import pytensor_config  # noqa: F401

# isort: split
import numpy as np
import pymc as pm

SeedFor = Callable[..., int]
"""``seed_for(request_seed, purpose[, index])`` derives a worker-specific sampler seed."""


def prior_predictive(config: dict[str, Any], seed_for: SeedFor) -> Any:
    """Draw ``config["prior_draws"]`` samples from the prior predictive."""
    return pm.sample_prior_predictive(
        draws=config["prior_draws"], random_seed=seed_for(config["seed"], "prior")
    )


def sample_nuts(config: dict[str, Any], chain_seeds: Sequence[int]) -> Any:
    """Run PyMC's NUTS sampler sequentially on one core with one seed per chain."""
    return pm.sample(
        draws=config["draws"],
        tune=config["tune"],
        chains=config["chains"],
        cores=1,
        blas_cores=1,
        random_seed=list(chain_seeds),
        target_accept=config["target_accept"],
        nuts_sampler="pymc",
        nuts={"max_treedepth": config["maximum_tree_depth"]},
        progressbar=False,
        quiet=True,
        return_inferencedata=True,
        compute_convergence_checks=False,
    )


def nuts(config: dict[str, Any], seed_for: SeedFor) -> Any:
    """``sample_nuts`` with chain ``i`` seeded by ``seed_for(seed, "chain", i)``."""
    chain_seeds = [seed_for(config["seed"], "chain", index) for index in range(config["chains"])]
    return sample_nuts(config, chain_seeds)


def posterior_predictive(
    posterior: Any, variable: str, config: dict[str, Any], seed_for: SeedFor
) -> Any:
    """Draw ``variable`` from the posterior predictive."""
    return pm.sample_posterior_predictive(
        posterior,
        var_names=[variable],
        random_seed=seed_for(config["seed"], "predictive"),
        progressbar=False,
    )


def sample_prior_posterior_predictive(
    config: dict[str, Any], seed_for: SeedFor, observed: str
) -> tuple[Any, Any, Any]:
    """Draw the prior predictive, the NUTS posterior, and the posterior predictive of
    ``observed``, in that order."""
    prior = prior_predictive(config, seed_for)
    posterior = nuts(config, seed_for)
    predictive = posterior_predictive(posterior, observed, config, seed_for)
    return prior, posterior, predictive


def prior_is_finite(prior: Any, groups: Sequence[str] = ("prior",)) -> bool:
    """Whether every variable in the named groups of a PyMC prior predictive is finite."""
    return bool(
        all(
            np.isfinite(np.asarray(value.values)).all()
            for group in groups
            for value in prior[group].values()
        )
    )
