"""MCMC convergence summaries shared by the PyMC and NumPyro workers.

The convergence summaries reduce per-parameter diagnostics to the worst value over the
monitored parameters, which is what the workers compare against the request's diagnostic
policy; ``mean_mcse`` instead reports one quantity's own Monte Carlo error.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

import arviz as az
import numpy as np


def flattened(dataset: Any, names: Sequence[str]) -> np.ndarray:
    """Concatenate the flattened ``.values`` of each named variable of an xarray-like mapping."""
    return np.concatenate([np.asarray(dataset[name].values).reshape(-1) for name in names])


def rank_convergence(posterior: Any, names: Sequence[str]) -> tuple[float, float, float]:
    """Return the maximum rank-normalized R-hat and the minimum bulk and tail ESS."""
    r_hat = float(flattened(az.rhat(posterior, var_names=names, method="rank"), names).max())
    bulk = float(flattened(az.ess(posterior, var_names=names, method="bulk"), names).min())
    tail = float(flattened(az.ess(posterior, var_names=names, method="tail"), names).min())
    return r_hat, bulk, tail


def mcse_extremes(posterior: Any, names: Sequence[str]) -> tuple[float, float]:
    """Return the maximum Monte Carlo standard error of the mean and of the SD."""
    mean = float(flattened(az.mcse(posterior, var_names=names, method="mean"), names).max())
    sd = float(flattened(az.mcse(posterior, var_names=names, method="sd"), names).max())
    return mean, sd


def mean_mcse(draws: Any, chains: int) -> float:
    """Return the Monte Carlo standard error of the mean of one scalar quantity's draws.

    ``draws`` holds ``chains`` equally long chains in chain-major order: a ``(chain, draw)``
    array or its flattening. Unlike ``mcse_extremes``, this is the quantity's own error, not
    the worst over the monitored parameters.
    """
    chain_draws = np.asarray(draws, dtype=np.float64).reshape(chains, -1)
    return float(az.mcse(chain_draws, method="mean"))


def minimum_ebfmi(energy: Any) -> float:
    """Return the smallest per-chain E-BFMI of a ``(chain, draw)`` Hamiltonian energy array."""
    energy = np.asarray(energy)
    per_chain = np.mean(np.diff(energy, axis=1) ** 2, axis=1) / np.var(energy, axis=1)
    return float(np.min(per_chain))


def divergence_count(diverging: Any) -> int:
    """Count divergent transitions from per-draw boolean flags.

    PyMC also records cumulative per-chain ``divergences`` counters; summing those would
    over-count, so callers pass the ``diverging`` flags.
    """
    return int(np.asarray(diverging).sum())


def pymc_sampler_diagnostics(sample_stats: Mapping[str, Any]) -> tuple[float, int, int]:
    """Return the minimum E-BFMI, divergences, and tree-depth hits from PyMC ``sample_stats``."""
    ebfmi = minimum_ebfmi(sample_stats["energy"].values)
    divergences = divergence_count(sample_stats["diverging"].values)
    depth_hits = int(np.asarray(sample_stats["reached_max_treedepth"].values).sum())
    return ebfmi, divergences, depth_hits


def tree_depth_hits(num_steps: Any, maximum_tree_depth: int) -> int:
    """Count NumPyro NUTS transitions that used all ``2**maximum_tree_depth - 1`` steps."""
    return int((np.asarray(num_steps) >= 2**maximum_tree_depth - 1).sum())
