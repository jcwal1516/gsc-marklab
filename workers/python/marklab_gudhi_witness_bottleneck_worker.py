#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import gudhi


GUDHI_VERSION = "3.13.0"


class ContractError(Exception):
    pass


def validate_diagram(diagram):
    finite = []
    for pair in diagram["finite_pairs"]:
        birth = float(pair["birth"])
        death = float(pair["death"])
        if not math.isfinite(birth) or not math.isfinite(death) or death < birth:
            raise ContractError("finite persistence pair is invalid")
        finite.append([birth, death])
    essential = []
    for value in diagram["essential_births"]:
        birth = float(value)
        if not math.isfinite(birth):
            raise ContractError("essential persistence birth is invalid")
        essential.append([birth, math.inf])
    return finite, essential


def main():
    if gudhi.__version__ != GUDHI_VERSION or sys.version_info[:2] != (3, 12):
        raise ContractError("GUDHI or Python version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if not request_bytes or len(request_bytes) > 16 * 1024 * 1024:
        raise ContractError("request is empty or too large")
    request = json.loads(request_bytes)
    if (
        request["format"] != "marklab.gudhi_witness_bottleneck_request"
        or request["version"] != 1
        or request["metric"] != "bottleneck_linf"
        or request["coefficient"] != 0.0
        or request["backend"]["name"] != "gudhi"
        or request["backend"]["version"] != GUDHI_VERSION
        or request["backend"]["python_version"] != "3.12"
    ):
        raise ContractError("request or backend identity mismatch")
    comparisons = request["comparisons"]
    if not comparisons or len(comparisons) > request["maximum_comparisons"]:
        raise ContractError("bottleneck comparison count exceeds maximum")
    interval_count = 0
    output = []
    maximum_finite = 0.0
    has_infinite = False
    seen = set()
    for comparison in comparisons:
        identity = (int(comparison["replicate"]), int(comparison["dimension"]))
        if identity in seen:
            raise ContractError("bottleneck comparison identity is duplicate")
        seen.add(identity)
        left_finite, left_essential = validate_diagram(comparison["baseline"])
        right_finite, right_essential = validate_diagram(comparison["perturbed"])
        interval_count += (
            len(left_finite)
            + len(left_essential)
            + len(right_finite)
            + len(right_essential)
        )
        if interval_count > request["maximum_interval_budget"]:
            raise ContractError("bottleneck interval budget exceeds maximum")
        if len(left_essential) != len(right_essential):
            status = "infinite_essential_count_mismatch"
            distance = None
            has_infinite = True
        else:
            distance = float(
                gudhi.bottleneck_distance(
                    left_finite + left_essential,
                    right_finite + right_essential,
                    e=0.0,
                )
            )
            if not math.isfinite(distance) or distance < 0.0:
                raise ContractError("bottleneck distance is non-finite or negative")
            maximum_finite = max(maximum_finite, distance)
            status = "finite"
        output.append(
            {
                "replicate": identity[0],
                "dimension": identity[1],
                "status": status,
                "bottleneck_distance_um_squared": distance,
            }
        )
    output.sort(key=lambda row: (row["replicate"], row["dimension"]))
    result = {
        "format": "marklab.gudhi_witness_bottleneck_result",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "metric": "gudhi_exact_linf",
        "essential_interval_policy": "infinite_death_equal_counts_else_infinite_mismatch",
        "comparisons": output,
        "comparison_count": len(output),
        "interval_count": interval_count,
        "maximum_finite_bottleneck_distance_um_squared": maximum_finite,
        "has_infinite_essential_mismatch": has_infinite,
        "claim_status": "exact_witness_diagram_bottleneck_stability_diagnostic",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(f"witness bottleneck worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
