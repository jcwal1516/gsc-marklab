import importlib.util
import math
import os
import unittest
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location(
    "projected_variogram_worker",
    ROOT / "workers/python/marklab_scipy_projected_variograms_worker.py",
)
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


class NativeProjectedVariogramTest(unittest.TestCase):
    def test_batched_curves_match_fsum_and_numpy_stratified_permutations(self):
        os.environ.setdefault(
            "MARKLAB_PROJECTED_VARIOGRAM_EXECUTABLE", str(ROOT / "target/debug/marklab")
        )
        coordinates = np.column_stack(([0, 0, 1, 2, 3, 4, 6, 10], np.zeros(8)))
        values = np.asarray(
            [
                [i / 3.0, (1e100 if i % 3 else 1e-100), (-1.0) ** i * 1e-100]
                for i in range(8)
            ]
        )
        bins = [
            {"lower_um": lower, "upper_um": upper, "upper_inclusive": index == 3}
            for index, (lower, upper) in enumerate(
                [(0, 0.5), (0.5, 0.75), (0.75, 3), (3, 10)]
            )
        ]
        request = {"bins": bins, "components": 3, "permutations": 35, "seed": 731}
        for biological_units in [["one"] * 8, ["a"] * 4 + ["b"] * 4]:
            strata = {}
            for index, unit in enumerate(biological_units):
                strata.setdefault((unit, str(index % 2)), []).append(index)

            observed, counts, null = worker.native_semivariograms(
                request, coordinates, values, strata, biological_units
            )

            pairs = [[] for _ in bins]
            for left in range(8):
                for right in range(left + 1, 8):
                    if biological_units[left] != biological_units[right]:
                        continue
                    distance = math.dist(coordinates[left], coordinates[right])
                    for index, distance_bin in enumerate(bins):
                        if distance >= distance_bin["lower_um"] and (
                            distance < distance_bin["upper_um"]
                            or (
                                distance_bin["upper_inclusive"]
                                and distance == distance_bin["upper_um"]
                            )
                        ):
                            pairs[index].append((left, right))
                            break

            def reference(permuted):
                result = np.full((3, len(bins)), np.nan)
                for component in range(3):
                    for bin_index, group in enumerate(pairs):
                        if group:
                            contributions = [
                                0.5
                                * float(
                                    np.square(
                                        permuted[left, component]
                                        - permuted[right, component]
                                    )
                                )
                                for left, right in group
                            ]
                            result[component, bin_index] = math.fsum(contributions) / len(
                                group
                            )
                return result

            np.testing.assert_array_equal(counts, [len(group) for group in pairs])
            np.testing.assert_array_equal(observed, reference(values))
            rng = np.random.Generator(np.random.PCG64(request["seed"]))
            for permutation in range(request["permutations"]):
                permuted = values.copy()
                for selected in strata.values():
                    selected = np.asarray(selected, dtype=np.int64)
                    permuted[selected] = values[rng.permutation(selected)]
                np.testing.assert_array_equal(null[permutation], reference(permuted))


if __name__ == "__main__":
    unittest.main()
