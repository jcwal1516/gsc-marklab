"""Known-answer tests for the Bayesian Thomas cluster-process sampler."""

import math
import sys
import unittest
from pathlib import Path

import numpy as np

sys.path.append(str(Path(__file__).resolve().parents[2] / "workers/python"))

import arviz as az  # noqa: E402

from marklab_worker_support import thomas_process  # noqa: E402

WINDOW = (0.0, 1.0, 0.0, 1.0)


class ThomasLikelihoodTest(unittest.TestCase):
    def test_incremental_state_matches_the_direct_likelihood_after_moves(self):
        rng = np.random.default_rng(5)
        points = thomas_process.simulate_pattern(
            rng, 20.0, 6.0, 0.04, WINDOW, 0.2, 10_000
        )
        state = thomas_process.PatternState(
            points,
            WINDOW,
            0.2,
            300,
            [math.log(20.0), math.log(6.0), math.log(0.04)],
            rng,
        )
        for _ in range(200):
            state.birth_or_death(rng, math.log(6.0), 1.0)
        state.displace_all(rng)
        state.update_parameter(rng, 2, 0.1, math.log(0.04), 1.0)
        parents = state.parents[: state.size]
        direct = thomas_process.log_likelihood(
            points, parents, state.mu, state.sigma, WINDOW
        )
        incremental = (
            len(points) * math.log(state.mu)
            + state.point_log_density()
            - state.mu * state.probability_sum
        )
        self.assertAlmostEqual(incremental, direct, delta=1e-9 * abs(direct))

    def test_edge_truncation_fraction_matches_quadrature(self):
        window = (0.0, 2.0, 0.0, 1.0)
        dilation = 0.15
        for sigma in (0.05, 0.1, 0.3):
            # Offspring intensity in W from parents in B, over that from all parents (|W|).
            grid = np.linspace(-dilation, 2.0 + dilation, 4001)
            inner_x = np.trapezoid(
                thomas_process.window_probability(
                    np.column_stack([grid, np.full_like(grid, 0.5)]),
                    sigma,
                    (0, 2, -1e9, 1e9),
                ),
                grid,
            )
            grid_y = np.linspace(-dilation, 1.0 + dilation, 4001)
            inner_y = np.trapezoid(
                thomas_process.window_probability(
                    np.column_stack([np.full_like(grid_y, 0.5), grid_y]),
                    sigma,
                    (-1e9, 1e9, 0, 1),
                ),
                grid_y,
            )
            expected = 1.0 - inner_x * inner_y / 2.0
            reported = float(
                thomas_process.edge_truncation_fraction(sigma, window, dilation)
            )
            self.assertAlmostEqual(reported, expected, delta=1e-7)


class OnePointPosteriorTest(unittest.TestCase):
    def test_parent_count_and_offspring_mean_match_quadrature(self):
        """With kappa and sigma fixed and one observed point, the posterior is analytic.

        Integrating the parents out gives ``p(log mu | x) ∝ Normal(log mu) mu A(mu)
        exp(kappa E(mu))`` and ``E[m | mu] = 1 + kappa E(mu)``, where ``A`` and ``E`` integrate
        ``N2(x; c, sigma^2 I) exp(-mu P(c))`` and ``exp(-mu P(c))`` over the parent region.
        """
        dilation, sigma, kappa = 0.3, 0.1, 5.0
        mu_mean, mu_sd = math.log(3.0), 0.5
        point = np.asarray([[0.3, 0.6]])
        region = thomas_process.parent_region(WINDOW, dilation)
        size = 600
        xs = region[0] + (np.arange(size) + 0.5) * (region[1] - region[0]) / size
        ys = region[2] + (np.arange(size) + 0.5) * (region[3] - region[2]) / size
        cell = thomas_process.box_area(region) / size**2
        cx, cy = np.meshgrid(xs, ys, indexing="ij")
        grid = np.column_stack([cx.ravel(), cy.ravel()])
        probability = thomas_process.window_probability(grid, sigma, WINDOW)
        kernel = thomas_process.gaussian_kernel(point, grid, sigma)[0]
        log_mu = np.linspace(mu_mean - 6 * mu_sd, mu_mean + 6 * mu_sd, 1201)
        log_density, parent_mean = [], []
        for value in log_mu:
            weight = np.exp(-math.exp(value) * probability)
            a_integral = float(np.sum(kernel * weight) * cell)
            e_integral = float(np.sum(weight) * cell)
            log_density.append(
                -0.5 * ((value - mu_mean) / mu_sd) ** 2
                + value
                + math.log(a_integral)
                + kappa * e_integral
            )
            parent_mean.append(1.0 + kappa * e_integral)
        weights = np.exp(np.asarray(log_density) - max(log_density))
        weights /= weights.sum()
        expected = {
            "parents": float(weights @ np.asarray(parent_mean)),
            "log_mu": float(weights @ log_mu),
        }
        draws = {"parents": [], "log_mu": []}
        for chain in range(4):
            rng = np.random.default_rng(100 + chain)
            state = thomas_process.PatternState(
                point,
                WINDOW,
                dilation,
                200,
                [math.log(kappa), mu_mean, math.log(sigma)],
                rng,
            )
            parents, mus = [], []
            for iteration in range(12_000):
                for _ in range(5):
                    state.birth_or_death(rng, mu_mean, mu_sd)
                state.displace_all(rng)
                state.update_parameter(rng, 1, 0.6, mu_mean, mu_sd)
                if iteration >= 1_000:
                    parents.append(state.size)
                    mus.append(float(state.theta[1]))
            draws["parents"].append(parents)
            draws["log_mu"].append(mus)
        for name, target in expected.items():
            values = np.asarray(draws[name], dtype=np.float64)
            error = float(az.mcse(values, method="mean"))
            # Four Monte Carlo standard errors, fixed before the comparison was run.
            self.assertLess(abs(values.mean() - target), 4.0 * error, name)


if __name__ == "__main__":
    unittest.main()
