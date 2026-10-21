#!/usr/bin/env python3
"""GUDHI worker for an exact stratified permutation test between persistence-diagram groups."""

import itertools
import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import gudhi

from marklab_worker_support.protocol import (
    CONTRACT_ERRORS,
    MIB,
    ContractError,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    write_result,
)


def energy(distance, treated):
    """Energy statistic between the treated diagrams and the rest under ``distance``."""
    treated = set(treated)
    left = sorted(treated)
    right = [index for index in range(len(distance)) if index not in treated]
    cross = sum(distance[i][j] for i in left for j in right)
    within_left = sum(distance[i][j] for i in left for j in left)
    within_right = sum(distance[i][j] for i in right for j in right)
    return (
        2.0 * cross / (len(left) * len(right))
        - within_left / len(left) ** 2
        - within_right / len(right) ** 2
    )


def main():
    require_versions(gudhi=gudhi.__version__)
    request_bytes = read_request(16 * MIB)
    request = json.loads(request_bytes)
    if (
        request["format"] != "marklab.gudhi_persistence_comparison_request"
        or request["version"] != 1
    ):
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
        "request_sha256": sha256_hex(request_bytes),
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
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "comparison worker contract error", errors=CONTRACT_ERRORS)
