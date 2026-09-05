"""Exercise each production diagnostic expression without running a sampler.

These workers currently keep extraction inside their large fit function. Executing the exact AST
expression lets a synthetic sample-statistics object exercise that boundary without copying the
formula or mocking the complete sampler/model. The fixture deliberately distinguishes per-draw
flags from PyMC 6.3's cumulative chain counters.
"""
import ast
from pathlib import Path
from types import SimpleNamespace
import unittest

import numpy as np

ROOT = Path(__file__).resolve().parents[2] / "workers/python"


def expressions():
    found = []
    for path in sorted(ROOT.glob("*.py")):
        tree = ast.parse(path.read_text())
        for node in ast.walk(tree):
            if not isinstance(node, ast.Assign):
                continue
            if not any(isinstance(target, ast.Name) and target.id in {"div", "divergences"} for target in node.targets):
                continue
            constants = {value.value for value in ast.walk(node.value) if isinstance(value, ast.Constant) and isinstance(value.value, str)}
            if "sample_stats" in constants and constants.intersection({"diverging", "divergences"}):
                expression = ast.fix_missing_locations(ast.Expression(node.value))
                found.append((path.name, compile(expression, str(path), "eval")))
    return found


class DivergenceCountTests(unittest.TestCase):
    def test_each_static_pymc_worker_counts_draws_not_cumulative_counters(self):
        computations = expressions()
        self.assertEqual(len(computations), 26, "re-evaluate the admitted worker inventory when it changes")
        for flags, expected in [([[False, True, False], [True, False, False]], 2), ([[False]*3]*2, 0), ([[True]*3]*2, 6)]:
            flags = np.asarray(flags, dtype=bool)
            posterior = {"sample_stats": {
                "diverging": SimpleNamespace(values=flags),
                "divergences": SimpleNamespace(values=np.cumsum(flags, axis=1)),
            }}
            for name, computation in computations:
                with self.subTest(worker=name, expected=expected):
                    actual = eval(computation, {"np": np, "posterior": posterior, "fit": posterior, "post": posterior})
                    self.assertEqual(actual, expected)
                    self.assertLessEqual(actual, flags.size)


if __name__ == "__main__":
    unittest.main()
