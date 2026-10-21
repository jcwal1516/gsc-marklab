"""Known answers for the pinned POT fused unbalanced Gromov-Wasserstein worker.

Tolerances were fixed before any solver comparison was run.
"""

import importlib.util
import math
import sys
import unittest
from pathlib import Path

import numpy as np
from scipy import optimize

WORKERS = Path(__file__).resolve().parents[2] / "workers/python"
sys.path.append(str(WORKERS))
SPEC = importlib.util.spec_from_file_location(
    "unbalanced_fgw_worker", WORKERS / "marklab_pot_unbalanced_fgw_worker.py"
)
worker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(worker)

IDENTITY_PLAN_TOLERANCE = 1e-6
SCALAR_RELATIVE_TOLERANCE = 1e-6
OBJECTIVE_RELATIVE_TOLERANCE = 1e-8
PLAN_TOLERANCE = 1e-4


def request(source_size, target_size, linear_weight, relaxation, epsilon=0.0):
    return {
        "source": [{"id": f"s{index}"} for index in range(source_size)],
        "target": [{"id": f"t{index}"} for index in range(target_size)],
        "linear_weight": linear_weight,
        "marginal_relaxation": relaxation,
        "epsilon": epsilon,
        "maximum_iterations": 2000,
        "tolerance": 1e-12,
        "maximum_inner_iterations": 20000,
        "inner_tolerance": 1e-14,
    }


def plan(fit, rows, columns):
    return np.asarray([entry["mass"] for entry in fit["plan"]]).reshape(rows, columns)


class UnbalancedFgwKnownAnswers(unittest.TestCase):
    def test_identical_spaces_are_matched_one_to_one_with_zero_objective(self):
        coordinates = np.asarray([[0.0, 0.0], [1.0, 0.0], [0.0, 2.0], [3.0, 1.0]])
        structure = np.linalg.norm(coordinates[:, None] - coordinates[None], axis=2)
        masses = np.asarray([0.1, 0.2, 0.3, 0.4])
        features = np.asarray([[0.0], [1.0], [2.0], [3.0]])
        cost = (features[:, None, 0] - features[None, :, 0]) ** 2
        problem = (masses, masses, cost, structure, structure)
        fits = worker.fit_all(request(4, 4, 1.0, 1.0), problem)
        fit = min(fits, key=lambda item: item["objective"]["total"])
        self.assertLess(
            np.max(np.abs(plan(fit, 4, 4) - np.diag(masses))), IDENTITY_PLAN_TOLERANCE
        )
        self.assertLess(abs(fit["objective"]["total"]), IDENTITY_PLAN_TOLERANCE)

    def test_single_points_reach_the_global_scalar_minimum(self):
        """For one source and one target point ``P = Q = p`` minimizes
        ``f(p) = a c p + rho (KL(p^2 | u^2) + KL(p^2 | v^2))``. Since
        ``f'(p) = a c + 4 rho p log(p^2 / (u v))`` is positive near zero, ``p = 0`` is a local
        minimum; an interior minimum is the larger root of ``f'``, in ``[sqrt(uv)/e, sqrt(uv)]``,
        and exists only when ``a c < 8 rho sqrt(uv) / e``. The expected plan is the global one;
        when that is the zero plan, the worker must report POT's coupling collapse.
        """
        for source_mass, target_mass, cost, weight, relaxation in [
            (0.6, 1.5, 0.0, 1.0, 0.5),
            (1.0, 1.0, 2.0, 0.5, 0.3),
            (0.2, 0.9, 0.7, 2.0, 1.7),
        ]:
            geometric = math.sqrt(source_mass * target_mass)

            def kl(value, reference):
                return (
                    value * math.log(value / reference) - value + reference
                    if value
                    else reference
                )

            def objective(value, cost=cost, weight=weight, relaxation=relaxation):
                square = value * value
                return weight * cost * value + relaxation * (
                    kl(square, source_mass**2) + kl(square, target_mass**2)
                )

            def stationarity(value, cost=cost, weight=weight, relaxation=relaxation):
                ratio = value * value / (source_mass * target_mass)
                return weight * cost + 4.0 * relaxation * value * math.log(ratio)

            candidates = [0.0]
            if cost == 0.0:
                candidates.append(geometric)
            elif stationarity(geometric / math.e) < 0.0:
                candidates.append(
                    optimize.brentq(
                        stationarity, geometric / math.e, geometric, xtol=1e-15
                    )
                )
            expected = min(candidates, key=objective)
            problem = (
                np.asarray([source_mass]),
                np.asarray([target_mass]),
                np.asarray([[cost]]),
                np.zeros((1, 1)),
                np.zeros((1, 1)),
            )
            if expected == 0.0:
                # POT's MM update divides a vanished coupling by itself; the worker reports
                # that collapse instead of returning a plan.
                with self.assertRaisesRegex(worker.ContractError, "collapsed"):
                    worker.fit_all(request(1, 1, weight, relaxation), problem)
                continue
            fits = worker.fit_all(request(1, 1, weight, relaxation), problem)
            fit = min(fits, key=lambda item: item["objective"]["total"])
            self.assertLess(
                abs(fit["transported_mass"] - expected),
                SCALAR_RELATIVE_TOLERANCE * max(1.0, expected),
                (source_mass, target_mass, cost, weight, relaxation, candidates),
            )

    def test_two_by_two_matches_an_independent_constrained_optimizer(self):
        """With zero structure the objective is linear plus tensorized marginal KL terms.

        SciPy SLSQP minimizes the documented objective over both couplings with equal mass,
        independently of POT's block-coordinate solver.
        """
        source = np.asarray([0.3, 0.7])
        target = np.asarray([0.5, 0.25])
        cost = np.asarray([[0.0, 1.0], [2.0, 0.5]])
        weight, relaxation = 0.8, 0.4
        zero = np.zeros((2, 2))
        problem = (source, target, cost, zero, zero)

        def objective(vector):
            first = vector[:4].reshape(2, 2)
            second = vector[4:].reshape(2, 2)
            return worker.objective_terms(
                first, second, source, target, cost, zero, zero, weight, relaxation, 0.0
            )["total"]

        best = None
        for start in (np.full(8, 0.1), np.tile(np.outer(source, target).ravel(), 2)):
            fitted = optimize.minimize(
                objective,
                start,
                method="SLSQP",
                bounds=[(0.0, None)] * 8,
                constraints=[
                    {"type": "eq", "fun": lambda v: v[:4].sum() - v[4:].sum()}
                ],
                options={"ftol": 1e-15, "maxiter": 2000},
            )
            if best is None or fitted.fun < best.fun:
                best = fitted
        fit = worker.solve(
            request(2, 2, weight, relaxation),
            "independent_mass",
            np.outer(source, target),
            problem,
        )
        reported = fit["objective"]["total"]
        self.assertLess(
            abs(reported - best.fun),
            OBJECTIVE_RELATIVE_TOLERANCE * max(1.0, abs(best.fun)),
        )
        self.assertLess(
            np.max(np.abs(plan(fit, 2, 2) - best.x[:4].reshape(2, 2))), PLAN_TOLERANCE
        )


if __name__ == "__main__":
    unittest.main()
