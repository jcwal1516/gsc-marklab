#!/usr/bin/env python3

import hashlib
import itertools
import json
import math
import sys

import gudhi


class ContractError(Exception):
    pass


def energy(distance, treated):
    treated = set(treated)
    left = sorted(treated)
    right = [index for index in range(len(distance)) if index not in treated]
    cross = sum(distance[i][j] for i in left for j in right)
    within_left = sum(distance[i][j] for i in left for j in left)
    within_right = sum(distance[i][j] for i in right for j in right)
    return 2.0 * cross / (len(left) * len(right)) - within_left / len(left) ** 2 - within_right / len(right) ** 2


def main():
    if gudhi.__version__ != "3.13.0" or sys.version_info[:2] != (3, 12):
        raise ContractError("GUDHI or Python version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.gudhi_persistence_comparison_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    diagrams = request["diagrams"]
    if len(diagrams) < 4 or request["metric"] != "bottleneck_linf":
        raise ContractError("comparison dimensions or metric are invalid")
    matrix = [[0.0] * len(diagrams) for _ in diagrams]
    for left in range(len(diagrams)):
        first = [[pair["birth"], pair["death"]] for pair in diagrams[left]["finite_pairs"]]
        for right in range(left + 1, len(diagrams)):
            second = [[pair["birth"], pair["death"]] for pair in diagrams[right]["finite_pairs"]]
            value = float(gudhi.bottleneck_distance(first, second, e=request["coefficient"]))
            if not math.isfinite(value):
                raise ContractError("bottleneck distance is non-finite")
            matrix[left][right] = value
            matrix[right][left] = value
    strata = {}
    for index, diagram in enumerate(diagrams):
        strata.setdefault(diagram["stratum"], []).append(index)
    choices = []
    for indices in strata.values():
        treated_count = sum(diagrams[index]["group"] == "A" for index in indices)
        choices.append(list(itertools.combinations(indices, treated_count)))
    assignment_count = math.prod(len(group) for group in choices)
    if assignment_count > request["maximum_exact_assignments"]:
        raise ContractError("exact assignment count exceeds caller maximum")
    observed_indices = [index for index, diagram in enumerate(diagrams) if diagram["group"] == "A"]
    observed = energy(matrix, observed_indices)
    null = []
    for assignment in itertools.product(*choices):
        treated = [index for selected in assignment for index in selected]
        null.append(energy(matrix, treated))
    p_value = sum(value >= observed - 1e-12 for value in null) / len(null)
    result = {
        "format": "marklab.persistence_distribution_comparison",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "patient_ids": [diagram["patient_id"] for diagram in diagrams],
        "groups": [diagram["group"] for diagram in diagrams],
        "metric": request["metric"],
        "permutation_unit": "whole_patient_diagram",
        "distance_matrix": matrix,
        "observed_energy": observed,
        "null_energies": null,
        "assignments_completed": len(null),
        "p_value_upper": p_value,
        "claim_status": "experimental_whole_patient_topology_comparison",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(f"comparison worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
