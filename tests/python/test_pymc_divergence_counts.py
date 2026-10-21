"""PyMC divergence counting must read per-draw flags, not PyMC 6.3's cumulative counters."""

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

import numpy as np

sys.path.append(str(Path(__file__).resolve().parents[2] / "workers" / "python"))

from marklab_worker_support.diagnostics import (
    divergence_count,
    pymc_sampler_diagnostics,
)

FLAG_CASES = [
    ([[False, True, False], [True, False, False]], 2),
    ([[False] * 3] * 2, 0),
    ([[True] * 3] * 2, 6),
]


def sample_stats(flags: np.ndarray) -> dict[str, SimpleNamespace]:
    """A PyMC-shaped ``sample_stats`` group with per-draw flags and cumulative counters."""
    chains, draws = flags.shape
    return {
        "diverging": SimpleNamespace(values=flags),
        "divergences": SimpleNamespace(values=np.cumsum(flags, axis=1)),
        "energy": SimpleNamespace(
            values=np.arange(chains * draws, dtype=float).reshape(flags.shape)
        ),
        "reached_max_treedepth": SimpleNamespace(values=np.zeros_like(flags)),
    }


class DivergenceCountTests(unittest.TestCase):
    def test_counts_divergent_draws(self):
        for flags, expected in FLAG_CASES:
            with self.subTest(expected=expected):
                self.assertEqual(
                    divergence_count(np.asarray(flags, dtype=bool)), expected
                )

    def test_pymc_diagnostics_ignore_cumulative_counters(self):
        for flags, expected in FLAG_CASES:
            flags = np.asarray(flags, dtype=bool)
            with self.subTest(expected=expected):
                _, divergences, depth_hits = pymc_sampler_diagnostics(
                    sample_stats(flags)
                )
                self.assertEqual(divergences, expected)
                self.assertLessEqual(divergences, flags.size)
                self.assertEqual(depth_hits, 0)


if __name__ == "__main__":
    unittest.main()
