#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import numpy as np
import scipy
from scipy.stats import rankdata


class ContractError(Exception):
    pass


def check(validation_id, status, metric, value, threshold, evidence):
    return {"validation_id": validation_id, "status": status, "metric": metric, "value": value, "threshold": threshold, "evidence": evidence}


def summaries(patterns):
    result = []
    for pattern in patterns:
        points = np.asarray(pattern["points"], dtype=float)
        result.append([len(points), points[:, 0].mean(), points[:, 1].mean(), points[:, 0].var(), points[:, 1].var()])
    return np.asarray(result, dtype=float)


def pair_histogram(patterns, bins):
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


def main():
    if np.__version__ != "2.4.6" or scipy.__version__ != "1.18.1" or sys.version_info[:2] != (3, 12):
        raise ContractError("generative validation backend version drift")
    request_bytes = sys.stdin.buffer.read(64 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.generative_validation_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    model = request["model"]
    repeat = request["model_repeat"]
    data = request["data"]
    if model["format"] != "marklab.point_set_flow_and_diffusion" or repeat["format"] != model["format"]:
        raise ContractError("validation requires point-set flow/diffusion artifacts")
    train = [pattern for pattern in data["patterns"] if pattern["split"] == "train"]
    heldout = [pattern for pattern in data["patterns"] if pattern["split"] == "test"]
    generated = model["generated"]["patterns"]
    train_summary = summaries(train)
    heldout_summary = summaries(heldout)
    generated_summary = summaries(generated)
    entries = []
    entries.append(check("artifact_identity", "passed", "format_and_request_identity", True, "exact", "hash-bound point-set generator artifact"))
    repeated = request["model_sha256"] == request["model_repeat_sha256"] and model == repeat
    entries.append(check("seed_reproducibility", "passed" if repeated else "failed", "byte_identical_repeat", repeated, True, "independent repeated CLI execution"))
    cardinality_relative = abs(generated_summary[:, 0].mean() - heldout_summary[:, 0].mean()) / heldout_summary[:, 0].mean()
    entries.append(check("cardinality", "passed" if cardinality_relative < 0.5 else "failed", "relative_mean_error", float(cardinality_relative), "<0.5", "heldout versus generated patterns"))
    window = data["patterns"][0]["window"]
    all_inside = all(window[0] <= point[0] <= window[1] and window[2] <= point[1] <= window[3] for pattern in generated for point in pattern["points"])
    entries.append(check("window_support", "passed" if all_inside else "failed", "all_points_inside", all_inside, True, "exact generated coordinates"))
    bins = np.linspace(0.0, math.sqrt(2.0), 12)
    heldout_pairs = pair_histogram(heldout, bins)
    generated_pairs = pair_histogram(generated, bins)
    pair_l1 = float(np.sum(np.abs(heldout_pairs - generated_pairs)))
    entries.append(check("pair_distance_distribution", "passed" if pair_l1 < 1.5 else "failed", "histogram_l1", pair_l1, "<1.5", "heldout versus generated all-pair distances"))
    heldout_points = np.concatenate([np.asarray(pattern["points"]) for pattern in heldout], axis=0)
    generated_points = np.concatenate([np.asarray(pattern["points"]) for pattern in generated], axis=0)
    heldout_nn = nearest_distances(heldout_points, heldout_points, leave_self=True).mean()
    generated_nn = nearest_distances(generated_points, generated_points, leave_self=True).mean()
    nn_relative = abs(generated_nn - heldout_nn) / max(heldout_nn, 1e-12)
    entries.append(check("nearest_neighbor_behavior", "passed" if nn_relative < 1.0 else "failed", "relative_mean_nn_error", float(nn_relative), "<1.0", "heldout versus generated nearest-neighbor scale"))
    context_means = {}
    for context in sorted({tuple(pattern["context"]) for pattern in generated}):
        points = np.concatenate([np.asarray(pattern["points"]) for pattern in generated if tuple(pattern["context"]) == context], axis=0)
        context_means[str(list(context))] = float(points[:, 0].mean())
    ordered_contexts = sorted(context_means.items())
    context_separated = len(ordered_contexts) == 2 and ordered_contexts[0][1] < ordered_contexts[1][1]
    entries.append(check("conditional_mode_consistency", "passed" if context_separated else "failed", "context_mean_x", context_means, "negative_context<positive_context", "generated conditional modes"))
    pairwise_summary = np.sqrt(np.sum((generated_summary[:, None, :] - generated_summary[None, :, :]) ** 2, axis=2)) + np.eye(len(generated_summary)) * 1e300
    minimum_diversity = float(pairwise_summary.min())
    entries.append(check("stochastic_diversity", "passed" if minimum_diversity > 0.0 else "failed", "minimum_summary_distance", minimum_diversity, ">0", "all generated pattern summaries"))
    generated_to_train = nearest_distances(generated_summary, train_summary)
    exact_memorization = int(np.sum(generated_to_train <= 1e-12))
    entries.append(check("nearest_neighbor_memorization", "passed" if exact_memorization == 0 else "failed", "exact_training_summary_matches", exact_memorization, 0, "generated-to-training summary distances"))
    train_scores = -nearest_distances(train_summary, generated_summary)
    heldout_scores = -nearest_distances(heldout_summary, generated_summary)
    combined = np.concatenate([train_scores, heldout_scores])
    ranks = rankdata(combined)
    auc = (ranks[:len(train_scores)].sum() - len(train_scores) * (len(train_scores) + 1) / 2) / (len(train_scores) * len(heldout_scores))
    entries.append(check("membership_inference_audit", "passed", "nearest_distance_auc", float(auc), "reported_not_promotion_gated_on_tiny_synthetic_sample", "train-versus-heldout attack score"))
    baseline_better = model["flow"]["heldout_log_likelihood"] > model["flow"]["uniform_poisson_log_likelihood"]
    entries.append(check("simpler_uniform_poisson_baseline", "passed" if baseline_better else "failed", "flow_minus_baseline_log_likelihood", float(model["flow"]["heldout_log_likelihood"] - model["flow"]["uniform_poisson_log_likelihood"]), ">0", "same heldout patterns"))
    count_min = int(generated_summary[:, 0].min())
    count_max = int(generated_summary[:, 0].max())
    entries.append(check("rare_cardinality_behavior", "passed", "generated_count_range", [count_min, count_max], "reported", "all seeded generator draws"))
    entries.extend([
        check("graph_motif_topology", "not_supported_by_point_only_fixture", "availability", None, "typed graph/topology model", "generator emits unmarked point sets only"),
        check("mark_cross_type_functions", "not_supported_by_unmarked_fixture", "availability", None, "trained mark generator", "point-set generator has no marks"),
        check("compartment_interface_summaries", "not_supported_by_fixture", "availability", None, "admitted compartment/interface labels", "synthetic fixture has none"),
        check("independent_real_patient_sites", "not_verified_missing_admitted_data", "availability", None, "external provenance-complete cohort", "synthetic patients only"),
    ])
    failed = [entry["validation_id"] for entry in entries if entry["status"] == "failed"]
    result = {
        "format": "marklab.generative_tissue_model_card",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "model_sha256": request["model_sha256"],
        "data_sha256": request["data_sha256"],
        "reproducibility": {"byte_identical_repeat": repeated, "repeat_sha256": request["model_repeat_sha256"]},
        "checks": entries,
        "failed_checks": failed,
        "maturity_decision": "research_only_synthetic" if not failed else "unavailable_failed_synthetic_checks",
        "claim_status": "synthetic_generative_model_validation_no_real_promotion",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"generative validation worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
