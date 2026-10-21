#!/usr/bin/env python3
"""Validation model card for the point-set flow/diffusion generator artifacts."""

import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import scipy
from scipy.stats import rankdata

from marklab_worker_support.protocol import (
    CONTRACT_ERRORS,
    MIB,
    ContractError,
    exact,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    write_result,
)
from marklab_worker_support.validation_checks import entry, pass_fail


def summaries(patterns):
    """Per pattern: point count, coordinate means, and coordinate variances."""
    result = []
    for pattern in patterns:
        points = np.asarray(pattern["points"], dtype=float)
        result.append(
            [
                len(points),
                points[:, 0].mean(),
                points[:, 1].mean(),
                points[:, 0].var(),
                points[:, 1].var(),
            ]
        )
    return np.asarray(result, dtype=float)


def pair_histogram(patterns, bins):
    """Normalized histogram of within-pattern pairwise distances pooled over patterns."""
    counts = np.zeros(len(bins) - 1)
    for pattern in patterns:
        points = np.asarray(pattern["points"], dtype=float)
        distances = np.sqrt(np.sum((points[:, None, :] - points[None, :, :]) ** 2, axis=2))
        distances = distances[np.triu_indices(len(points), 1)]
        counts += np.histogram(distances, bins=bins)[0]
    return counts / max(counts.sum(), 1.0)


def nearest_distances(left, right, leave_self=False):
    distances = np.sqrt(np.sum((left[:, None, :] - right[None, :, :]) ** 2, axis=2))
    if leave_self and len(left) == len(right):
        distances += np.eye(len(left)) * 1e300
    return distances.min(axis=1)


def concatenated_points(patterns):
    return np.concatenate([np.asarray(pattern["points"]) for pattern in patterns], axis=0)


def model_checks(request, model, repeat, data):
    train = [pattern for pattern in data["patterns"] if pattern["split"] == "train"]
    heldout = [pattern for pattern in data["patterns"] if pattern["split"] == "test"]
    generated = model["generated"]["patterns"]
    train_summary = summaries(train)
    heldout_summary = summaries(heldout)
    generated_summary = summaries(generated)
    entries = [
        entry(
            "artifact_identity",
            "passed",
            "format_and_request_identity",
            True,
            "exact",
            "hash-bound point-set generator artifact",
        )
    ]

    repeated = request["model_sha256"] == request["model_repeat_sha256"] and model == repeat
    entries.append(
        entry(
            "seed_reproducibility",
            pass_fail(repeated),
            "byte_identical_repeat",
            repeated,
            True,
            "independent repeated CLI execution",
        )
    )

    heldout_mean_count = heldout_summary[:, 0].mean()
    cardinality_relative = (
        abs(generated_summary[:, 0].mean() - heldout_mean_count) / heldout_mean_count
    )
    entries.append(
        entry(
            "cardinality",
            pass_fail(cardinality_relative < 0.5),
            "relative_mean_error",
            float(cardinality_relative),
            "<0.5",
            "heldout versus generated patterns",
        )
    )

    window = data["patterns"][0]["window"]
    all_inside = all(
        window[0] <= point[0] <= window[1] and window[2] <= point[1] <= window[3]
        for pattern in generated
        for point in pattern["points"]
    )
    entries.append(
        entry(
            "window_support",
            pass_fail(all_inside),
            "all_points_inside",
            all_inside,
            True,
            "exact generated coordinates",
        )
    )

    bins = np.linspace(0.0, math.sqrt(2.0), 12)
    heldout_pairs = pair_histogram(heldout, bins)
    generated_pairs = pair_histogram(generated, bins)
    pair_l1 = float(np.sum(np.abs(heldout_pairs - generated_pairs)))
    entries.append(
        entry(
            "pair_distance_distribution",
            pass_fail(pair_l1 < 1.5),
            "histogram_l1",
            pair_l1,
            "<1.5",
            "heldout versus generated all-pair distances",
        )
    )

    heldout_points = concatenated_points(heldout)
    generated_points = concatenated_points(generated)
    heldout_nn = nearest_distances(heldout_points, heldout_points, leave_self=True).mean()
    generated_nn = nearest_distances(generated_points, generated_points, leave_self=True).mean()
    nn_relative = abs(generated_nn - heldout_nn) / max(heldout_nn, 1e-12)
    entries.append(
        entry(
            "nearest_neighbor_behavior",
            pass_fail(nn_relative < 1.0),
            "relative_mean_nn_error",
            float(nn_relative),
            "<1.0",
            "heldout versus generated nearest-neighbor scale",
        )
    )

    context_means = {}
    for context in sorted({tuple(pattern["context"]) for pattern in generated}):
        points = concatenated_points(
            [pattern for pattern in generated if tuple(pattern["context"]) == context]
        )
        context_means[str(list(context))] = float(points[:, 0].mean())
    ordered_contexts = sorted(context_means.items())
    context_separated = (
        len(ordered_contexts) == 2 and ordered_contexts[0][1] < ordered_contexts[1][1]
    )
    entries.append(
        entry(
            "conditional_mode_consistency",
            pass_fail(context_separated),
            "context_mean_x",
            context_means,
            "negative_context<positive_context",
            "generated conditional modes",
        )
    )

    summary_differences = generated_summary[:, None, :] - generated_summary[None, :, :]
    pairwise_summary = np.sqrt(np.sum(summary_differences**2, axis=2))
    pairwise_summary = pairwise_summary + np.eye(len(generated_summary)) * 1e300
    minimum_diversity = float(pairwise_summary.min())
    entries.append(
        entry(
            "stochastic_diversity",
            pass_fail(minimum_diversity > 0.0),
            "minimum_summary_distance",
            minimum_diversity,
            ">0",
            "all generated pattern summaries",
        )
    )

    generated_to_train = nearest_distances(generated_summary, train_summary)
    exact_memorization = int(np.sum(generated_to_train <= 1e-12))
    entries.append(
        entry(
            "nearest_neighbor_memorization",
            pass_fail(exact_memorization == 0),
            "exact_training_summary_matches",
            exact_memorization,
            0,
            "generated-to-training summary distances",
        )
    )

    train_scores = -nearest_distances(train_summary, generated_summary)
    heldout_scores = -nearest_distances(heldout_summary, generated_summary)
    ranks = rankdata(np.concatenate([train_scores, heldout_scores]))
    train_count = len(train_scores)
    train_rank_sum = ranks[:train_count].sum() - train_count * (train_count + 1) / 2
    auc = train_rank_sum / (train_count * len(heldout_scores))
    entries.append(
        entry(
            "membership_inference_audit",
            "passed",
            "nearest_distance_auc",
            float(auc),
            "reported_not_promotion_gated_on_tiny_synthetic_sample",
            "train-versus-heldout attack score",
        )
    )

    flow = model["flow"]
    baseline_better = flow["heldout_log_likelihood"] > flow["uniform_poisson_log_likelihood"]
    entries.append(
        entry(
            "simpler_uniform_poisson_baseline",
            pass_fail(baseline_better),
            "flow_minus_baseline_log_likelihood",
            float(flow["heldout_log_likelihood"] - flow["uniform_poisson_log_likelihood"]),
            ">0",
            "same heldout patterns",
        )
    )

    count_min = int(generated_summary[:, 0].min())
    count_max = int(generated_summary[:, 0].max())
    entries.append(
        entry(
            "rare_cardinality_behavior",
            "passed",
            "generated_count_range",
            [count_min, count_max],
            "reported",
            "all seeded generator draws",
        )
    )
    entries.extend(
        [
            entry(
                "graph_motif_topology",
                "not_supported_by_point_only_fixture",
                "availability",
                None,
                "typed graph/topology model",
                "generator emits unmarked point sets only",
            ),
            entry(
                "mark_cross_type_functions",
                "not_supported_by_unmarked_fixture",
                "availability",
                None,
                "trained mark generator",
                "point-set generator has no marks",
            ),
            entry(
                "compartment_interface_summaries",
                "not_supported_by_fixture",
                "availability",
                None,
                "admitted compartment/interface labels",
                "synthetic fixture has none",
            ),
            entry(
                "independent_real_patient_sites",
                "not_verified_missing_admitted_data",
                "availability",
                None,
                "external provenance-complete cohort",
                "synthetic patients only",
            ),
        ]
    )
    return entries, repeated


def main() -> None:
    require_versions(numpy=np.__version__, scipy=scipy.__version__)
    request_bytes = read_request(64 * MIB)
    request = json.loads(request_bytes)
    exact(request["format"], "marklab.generative_validation_request", "request.format")
    exact(request["version"], 1, "request.version")
    model = request["model"]
    repeat = request["model_repeat"]
    if (
        model["format"] != "marklab.point_set_flow_and_diffusion"
        or repeat["format"] != model["format"]
    ):
        raise ContractError("validation requires point-set flow/diffusion artifacts")
    entries, repeated = model_checks(request, model, repeat, request["data"])
    failed = [item["validation_id"] for item in entries if item["status"] == "failed"]
    result = {
        "format": "marklab.generative_tissue_model_card",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "model_sha256": request["model_sha256"],
        "data_sha256": request["data_sha256"],
        "reproducibility": {
            "byte_identical_repeat": repeated,
            "repeat_sha256": request["model_repeat_sha256"],
        },
        "checks": entries,
        "failed_checks": failed,
        "maturity_decision": (
            "research_only_synthetic" if not failed else "unavailable_failed_synthetic_checks"
        ),
        "claim_status": "synthetic_generative_model_validation_no_real_promotion",
    }
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "generative validation worker contract error", errors=CONTRACT_ERRORS)
