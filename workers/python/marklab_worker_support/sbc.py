"""Simulation-based calibration (SBC) summaries shared by the SBC workers.

For each replicate a worker draws parameters from the prior, simulates data, fits the model,
and records the rank of the true value among the posterior draws and whether the central 90%
posterior interval covers it. Under a calibrated model and sampler the ranks are uniform on
``0..draws`` and the coverage is close to 0.9.
"""

from __future__ import annotations

import math
from collections.abc import Iterable, Mapping, Sequence
from typing import Any

import numpy as np
from scipy import stats

EXACT_MULTINOMIAL_MAXIMUM_COUNT = 50
EXACT_MULTINOMIAL_MAXIMUM_BINS = 20
MONTE_CARLO_REPLICATES = 20_000
MONTE_CARLO_SEED = 0x6A09E667F3BCC909
_MASK64 = (1 << 64) - 1


def rank_and_coverage(draws: Any, truth: float) -> tuple[int, bool]:
    """Return the number of posterior draws below ``truth`` and 90%-interval coverage."""
    draws = np.asarray(draws)
    rank = int(np.sum(draws < truth))
    covered = bool(np.quantile(draws, 0.05) <= truth <= np.quantile(draws, 0.95))
    return rank, covered


def rank_histogram(ranks: Sequence[int], draws: int, bins: int) -> np.ndarray:
    """Bin ranks in ``0..draws`` into ``bins`` equal-width bins."""
    histogram = np.zeros(bins, dtype=np.int64)
    for rank in np.asarray(ranks, dtype=np.int64):
        histogram[min(int(rank) * bins // (draws + 1), bins - 1)] += 1
    return histogram


def rank_uniformity(histogram: np.ndarray, draws: int) -> tuple[float, float]:
    """Pearson statistic and a finite-sample-safe p-value for discrete-uniform ranks.

    Sparse histograms with at most 50 replicates and 20 bins use the exact multinomial tail;
    other sparse histograms use a deterministic, 20,000-sample Monte Carlo tail. The chi-square
    approximation is used only when every expected bin count is at least five.
    """
    histogram = np.asarray(histogram, dtype=np.int64)
    bins = len(histogram)
    support = draws + 1
    # ceil(j * support / bins) is the first integer rank in bin j. The bin
    # probabilities are equal only when bins divides support exactly.
    edges = (np.arange(bins + 1, dtype=np.int64) * support + bins - 1) // bins
    bin_sizes = np.diff(edges)
    expected = histogram.sum() * bin_sizes / support
    statistic = float(np.sum(np.square(histogram - expected) / expected))
    if np.all(expected >= 5.0):
        p_value = float(stats.chi2.sf(statistic, bins - 1))
    elif (
        int(histogram.sum()) <= EXACT_MULTINOMIAL_MAXIMUM_COUNT
        and bins <= EXACT_MULTINOMIAL_MAXIMUM_BINS
    ):
        p_value = _exact_multinomial_tail(histogram, bin_sizes, support)
    else:
        p_value = _deterministic_multinomial_tail(histogram, bin_sizes, support)
    return statistic, p_value


def _score_scale(bin_sizes: np.ndarray) -> int:
    scale = 1
    for size in bin_sizes:
        scale = math.lcm(scale, int(size))
    return scale


def _histogram_score(histogram: Sequence[int], bin_sizes: np.ndarray, scale: int) -> int:
    return sum(
        int(count) ** 2 * (scale // int(size))
        for count, size in zip(histogram, bin_sizes, strict=True)
    )


def _exact_multinomial_tail(histogram: np.ndarray, bin_sizes: np.ndarray, support: int) -> float:
    count = int(histogram.sum())
    scale = _score_scale(bin_sizes)
    observed_score = _histogram_score(histogram, bin_sizes, scale)
    states: dict[tuple[int, int], float] = {(0, 0): 1.0}
    for size_value in bin_sizes:
        size = int(size_value)
        probability = size / support
        multiplier = scale // size
        terms = [1.0]
        for value in range(1, count + 1):
            terms.append(terms[-1] * probability / value)
        next_states: dict[tuple[int, int], float] = {}
        for (used, score), weight in sorted(states.items()):
            for added in range(count - used + 1):
                key = (used + added, score + added * added * multiplier)
                next_states[key] = next_states.get(key, 0.0) + weight * terms[added]
        states = next_states
    coefficient = sum(
        weight
        for (used, score), weight in states.items()
        if used == count and score >= observed_score
    )
    probability = coefficient * math.factorial(count)
    return 1.0 if probability >= 1.0 - 1e-12 else min(1.0, max(0.0, probability))


def _splitmix64(state: int) -> tuple[int, int]:
    state = (state + 0x9E3779B97F4A7C15) & _MASK64
    value = state
    value = ((value ^ (value >> 30)) * 0xBF58476D1CE4E5B9) & _MASK64
    value = ((value ^ (value >> 27)) * 0x94D049BB133111EB) & _MASK64
    return state, value ^ (value >> 31)


def _deterministic_multinomial_tail(
    histogram: np.ndarray, bin_sizes: np.ndarray, support: int
) -> float:
    count = int(histogram.sum())
    bins = len(histogram)
    scale = _score_scale(bin_sizes)
    observed_score = _histogram_score(histogram, bin_sizes, scale)
    state = MONTE_CARLO_SEED
    rejection_threshold = ((-support) & _MASK64) % support
    extreme = 0
    simulated: dict[int, int] = {}
    for _ in range(MONTE_CARLO_REPLICATES):
        simulated.clear()
        for _ in range(count):
            while True:
                state, candidate = _splitmix64(state)
                if candidate >= rejection_threshold:
                    rank = candidate % support
                    break
            bin_index = min(rank * bins // support, bins - 1)
            simulated[bin_index] = simulated.get(bin_index, 0) + 1
        simulated_score = sum(
            count_value * count_value * (scale // int(bin_sizes[bin_index]))
            for bin_index, count_value in simulated.items()
        )
        if simulated_score >= observed_score:
            extreme += 1
    return (extreme + 1) / (MONTE_CARLO_REPLICATES + 1)


def maximum_rank_ecdf_deviation(ranks: Sequence[int], draws: int) -> float:
    """Compare both discrete CDFs after each possible rank, including unobserved ranks."""
    counts = np.bincount(ranks, minlength=draws + 1)
    empirical = counts.cumsum() / len(ranks)
    expected = np.arange(1, draws + 2) / (draws + 1)
    return float(np.max(np.abs(empirical - expected)))


def rank_summary(
    ranks: Sequence[int], covered: Sequence[bool], draws: int, bins: int
) -> dict[str, Any]:
    """Summarize one parameter's SBC ranks and coverage flags over the completed replicates.

    The uniformity p-value uses the exact bin probabilities of a discrete-uniform rank and a
    finite-sample multinomial tail when expected counts are sparse. All statistics are zero when
    no replicate completed.
    """
    ranks = np.asarray(ranks, dtype=np.int64)
    histogram = rank_histogram(ranks, draws, bins)
    completed = len(ranks) > 0
    return {
        "rank_histogram": histogram.tolist(),
        "rank_uniformity_p_value": rank_uniformity(histogram, draws)[1] if completed else 0.0,
        "coverage_90": float(np.mean(covered)) if completed else 0.0,
        "mean_normalized_rank": float(ranks.mean() / draws) if completed else 0.0,
    }


def calibration_passes(
    summaries: Iterable[Mapping[str, Any]],
    failures: Sequence[Any],
    minimum_rank_p: float,
    minimum_coverage: float,
    maximum_coverage: float,
) -> bool:
    """Return whether every replicate completed and every summary meets the policy."""
    return not failures and all(
        minimum_rank_p <= summary["rank_uniformity_p_value"]
        and minimum_coverage <= summary["coverage_90"] <= maximum_coverage
        for summary in summaries
    )
