"""NumPyro NUTS runs, convergence summaries, and agreement-request handling.

Every NumPyro worker runs its chains sequentially and records the ``diverging``, ``num_steps``,
and ``energy`` extra fields, from which ``nuts_diagnostics`` derives the same worst-case summaries
that the PyMC workers report. Agreement workers re-fit a model whose request contract is owned
by a PyMC worker; ``validate_source_request`` checks the wrapping request and delegates the
embedded source request to that worker's ``validate``.

Import only after enabling JAX 64-bit mode; the workers do so before importing NumPyro.
"""

from __future__ import annotations

from collections.abc import Callable, Collection, Sequence
from types import ModuleType
from typing import Any

import arviz as az
import jax
import numpy as np
import numpyro
from numpyro.infer import MCMC, NUTS, Predictive

from . import versions
from .diagnostics import (
    divergence_count,
    mcse_extremes,
    minimum_ebfmi,
    rank_convergence,
    tree_depth_hits,
)
from .protocol import (
    backend_identity,
    exact,
    file_sha256,
    obj,
    sha256_digest,
    validate_backend,
)

EXTRA_FIELDS = ("diverging", "num_steps", "energy")
SOURCE_KEYS = frozenset(
    {"format", "version", "backend", "jax_version", "source_request_sha256", "source_request"}
)


def run_chains(
    kernel: NUTS, config: dict[str, Any], rng_key: Any, arguments: dict[str, Any] | None = None
) -> tuple[MCMC, dict[str, np.ndarray]]:
    """Run ``config["chains"]`` sequential chains of ``config["tune"]`` warmup and
    ``config["draws"]`` kept iterations; return the sampler and float64 samples by chain."""
    sampler = MCMC(
        kernel,
        num_warmup=config["tune"],
        num_samples=config["draws"],
        num_chains=config["chains"],
        chain_method="sequential",
        progress_bar=False,
    )
    sampler.run(rng_key, **(arguments or {}), extra_fields=EXTRA_FIELDS)
    samples = {
        name: np.asarray(value, dtype=np.float64)
        for name, value in sampler.get_samples(group_by_chain=True).items()
    }
    return sampler, samples


def sample_nuts(
    model: Callable[[], None], config: dict[str, Any], maximum_tree_depth: int, seed: int
) -> tuple[dict[str, np.ndarray], dict[str, Any]]:
    """Run NUTS (diagonal mass matrix) on an argument-free model; return the float64 samples
    and the extra fields, both grouped by chain."""
    kernel = NUTS(
        model, target_accept_prob=config["target_accept"], max_tree_depth=maximum_tree_depth
    )
    sampler, samples = run_chains(kernel, config, jax.random.PRNGKey(seed))
    return samples, sampler.get_extra_fields(group_by_chain=True)


def nuts_diagnostics(
    samples: dict[str, np.ndarray],
    names: Sequence[str],
    extra: dict[str, Any],
    maximum_tree_depth: int,
    *,
    mcse: bool = False,
) -> dict[str, Any]:
    """Worst-case convergence diagnostics over the named sites of chain-grouped samples.

    Reports the maximum rank R-hat, minimum bulk and tail ESS, optionally the maximum MCSE of
    the mean and SD, the minimum E-BFMI, and the divergence and maximum-tree-depth counts.
    """
    posterior = az.from_dict({"posterior": {name: samples[name] for name in names}})
    r_hat, bulk, tail = rank_convergence(posterior, names)
    diagnostics: dict[str, Any] = {"r_hat": r_hat, "ess_bulk": bulk, "ess_tail": tail}
    if mcse:
        diagnostics["mcse_mean"], diagnostics["mcse_sd"] = mcse_extremes(posterior, names)
    diagnostics["minimum_ebfmi"] = minimum_ebfmi(np.asarray(extra["energy"], dtype=np.float64))
    diagnostics["divergences"] = divergence_count(extra["diverging"])
    diagnostics["max_tree_depth_hits"] = tree_depth_hits(extra["num_steps"], maximum_tree_depth)
    return diagnostics


def validate_source_request(
    request: Any,
    request_format: str,
    lock_sha: str,
    worker_sha: str,
    pymc_contract: ModuleType,
    extra_keys: Collection[str] = (),
) -> tuple[dict[str, Any], dict[str, Any], str]:
    """Validate a NumPyro request that wraps a PyMC source request.

    The source request is validated by ``pymc_contract.validate`` with the PyMC worker file's
    own digest. Returns the request, the parsed configuration, and the source request digest.
    """
    request = obj(request, SOURCE_KEYS | set(extra_keys), "request")
    exact(request["format"], request_format, "request.format")
    exact(request["version"], 1, "request.version")
    validate_backend(
        request["backend"],
        "backend",
        name="numpyro",
        version=versions.NUMPYRO,
        lock_sha256=lock_sha,
        worker_sha256=worker_sha,
    )
    exact(request["jax_version"], versions.JAX, "request.jax_version")
    source_sha = sha256_digest(request["source_request_sha256"], "request.source_request_sha256")
    config = pymc_contract.validate(
        request["source_request"], lock_sha, file_sha256(pymc_contract.__file__)
    )
    return request, config, source_sha


def result_header(
    result_format: str,
    lock_sha: str,
    worker_sha: str,
    request_sha: str,
    source_sha: str,
) -> dict[str, Any]:
    """The identity fields that open every NumPyro agreement result."""
    return {
        "format": result_format,
        "version": 1,
        "backend": backend_identity("numpyro", numpyro.__version__, lock_sha, worker_sha),
        "jax_version": jax.__version__,
        "request_sha256": request_sha,
        "source_request_sha256": source_sha,
    }


def fit_with_predictive(
    model: Callable[..., None],
    config: dict[str, Any],
    seed_for: Callable[[int, str], int],
    observed_argument: str,
    observed: Any,
    arguments: dict[str, Any],
    predictive_site: str,
) -> tuple[dict[str, Any], MCMC, dict[str, np.ndarray], np.ndarray]:
    """Draw the prior predictive, fit by dense-mass NUTS, and replicate the data.

    ``observed_argument`` names the model argument that receives ``observed`` (``None`` when
    simulating). Returns the prior predictive draws, the sampler, the float64 posterior samples
    grouped by chain, and the integer posterior predictive draws of ``predictive_site``.
    """
    prior = Predictive(model, num_samples=config["prior_draws"])(
        jax.random.PRNGKey(seed_for(config["seed"], "prior")),
        **{observed_argument: None},
        **arguments,
    )
    kernel = NUTS(
        model,
        target_accept_prob=config["target_accept"],
        max_tree_depth=config["maximum_tree_depth"],
        dense_mass=True,
    )
    sampler, chain_samples = run_chains(
        kernel,
        config,
        jax.random.PRNGKey(seed_for(config["seed"], "sample")),
        {observed_argument: observed, **arguments},
    )
    predictive = Predictive(
        model,
        posterior_samples=sampler.get_samples(group_by_chain=False),
        return_sites=[predictive_site],
    )
    replicated = predictive(
        jax.random.PRNGKey(seed_for(config["seed"], "predictive")),
        **{observed_argument: None},
        **arguments,
    )[predictive_site]
    return prior, sampler, chain_samples, np.asarray(replicated, dtype=np.int64)


def prior_is_finite(prior: dict[str, Any]) -> bool:
    """Whether every site of a NumPyro prior predictive draw is finite."""
    return bool(all(np.isfinite(np.asarray(value)).all() for value in prior.values()))


def posterior_is_finite(chain_samples: dict[str, np.ndarray], replicated: np.ndarray) -> bool:
    """Whether every posterior sample site and every replicated observation is finite."""
    return bool(
        all(np.isfinite(value).all() for value in chain_samples.values())
        and np.isfinite(replicated).all()
    )
