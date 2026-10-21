"""Ordinal-outcome pieces shared by the PyMC patient ordinal workers.

The proportional-odds, site-hierarchy, and non-proportional ordinal workers model one ordered
outcome per patient with ordered cutpoints on the logit scale, report per-level reference and
comparison group probabilities, and check them against the observed level proportions.
"""

from __future__ import annotations

from typing import Any

from . import pytensor_config  # noqa: F401

# isort: split
import numpy as np
import pymc as pm

from .posterior import clipped_sigmoid, summary


def initial_cutpoints(levels: list[str]) -> np.ndarray:
    """Evenly spaced cutpoints on ``[-1.5, 1.5]``: the cutpoint prior mean and initial value."""
    return np.linspace(-1.5, 1.5, len(levels) - 1)


def ordered_cutpoints(name: str, initial: np.ndarray, sd: float) -> Any:
    """Normal prior densities with means ``initial`` and SD ``sd``, restricted to ordered values.

    The ordered transform enforces increasing cutpoints; the resulting cutpoints are not
    independent under that constraint.
    """
    return pm.Normal(
        name,
        mu=initial,
        sigma=sd,
        shape=len(initial),
        transform=pm.distributions.transforms.ordered,
        initval=initial,
    )


def category_probabilities(cutpoints: np.ndarray, eta: np.ndarray) -> np.ndarray:
    """Ordered-logistic level probabilities ``P(y = k) = F(c_k - eta) - F(c_{k-1} - eta)``.

    The last axis of ``cutpoints`` holds the cutpoints; ``eta`` broadcasts against the others.
    """
    cumulative = clipped_sigmoid(cutpoints - eta[..., None])
    edge_shape = (*cumulative.shape[:-1], 1)
    bounded = np.concatenate([np.zeros(edge_shape), cumulative, np.ones(edge_shape)], axis=-1)
    return np.diff(bounded, axis=-1)


def cutpoint_summaries(levels: list[str], cutpoint_draws: np.ndarray) -> list[dict[str, Any]]:
    """Posterior summary of each cutpoint, labelled by the levels it separates."""
    return [
        {
            "lower_level": levels[index],
            "upper_level": levels[index + 1],
            **summary(cutpoint_draws[..., index]),
        }
        for index in range(len(levels) - 1)
    ]


def level_reports(
    levels: list[str],
    outcomes: np.ndarray,
    comparison_mask: np.ndarray,
    reference_probabilities: np.ndarray,
    comparison_probabilities: np.ndarray,
    replicated: np.ndarray,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Per-level posterior probabilities and posterior predictive checks.

    ``*_probabilities`` are ``(draw, level)`` group probabilities and ``replicated`` is
    ``(draw, patient)`` replicated outcome codes. The check compares the summed absolute gap
    between replicated and modelled group proportions with the observed gap.
    """
    reference_mask = ~comparison_mask
    posteriors = []
    checks = []
    for index, level in enumerate(levels):
        reference = reference_probabilities[:, index]
        comparison = comparison_probabilities[:, index]
        observed_reference = float(np.mean(outcomes[reference_mask] == index))
        observed_comparison = float(np.mean(outcomes[comparison_mask] == index))
        replicated_reference = np.mean(replicated[:, reference_mask] == index, axis=1)
        replicated_comparison = np.mean(replicated[:, comparison_mask] == index, axis=1)
        observed_error = np.abs(observed_reference - reference) + np.abs(
            observed_comparison - comparison
        )
        replicated_error = np.abs(replicated_reference - reference) + np.abs(
            replicated_comparison - comparison
        )
        posteriors.append(
            {
                "level": level,
                "reference_probability": summary(reference),
                "comparison_probability": summary(comparison),
                "difference_comparison_minus_reference": summary(comparison - reference),
            }
        )
        checks.append(
            {
                "level": level,
                "observed_reference_proportion": observed_reference,
                "observed_comparison_proportion": observed_comparison,
                "replicated_reference_proportion_mean": float(replicated_reference.mean()),
                "replicated_comparison_proportion_mean": float(replicated_comparison.mean()),
                "probability_absolute_replication_error_at_least_observed": float(
                    np.mean(replicated_error >= observed_error)
                ),
            }
        )
    return posteriors, checks
