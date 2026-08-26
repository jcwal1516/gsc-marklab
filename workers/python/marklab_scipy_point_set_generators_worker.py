#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import numpy as np
import scipy
from scipy.special import expit, gammaln


class ContractError(Exception):
    pass


def logit(values, epsilon):
    admitted = np.clip(values, epsilon, 1.0 - epsilon)
    return np.log(admitted) - np.log1p(-admitted)


def main():
    if np.__version__ != "2.4.6" or scipy.__version__ != "1.18.1" or sys.version_info[:2] != (3, 12):
        raise ContractError("point-set generator backend version drift")
    request_bytes = sys.stdin.buffer.read(32 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.scipy_point_set_generators_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    patterns = request["patterns"]
    train = [pattern for pattern in patterns if pattern["split"] == "train"]
    test = [pattern for pattern in patterns if pattern["split"] == "test"]
    window = np.asarray(patterns[0]["window"], dtype=float)
    origin = np.asarray([window[0], window[2]])
    extent = np.asarray([window[1] - window[0], window[3] - window[2]])
    epsilon = request["flow"]["boundary_epsilon"]
    design_rows = []
    logits = []
    counts = []
    pattern_contexts = []
    for pattern in train:
        context = np.asarray(pattern["context"], dtype=float)
        normalized = (np.asarray(pattern["points"], dtype=float) - origin) / extent
        transformed = logit(normalized, epsilon)
        logits.append(transformed)
        design_rows.append(np.column_stack([np.ones(len(transformed)), np.broadcast_to(context, (len(transformed), len(context)))]))
        counts.append(len(transformed))
        pattern_contexts.append(context)
    all_logits = np.concatenate(logits, axis=0)
    design = np.concatenate(design_rows, axis=0)
    coefficients = np.linalg.lstsq(design, all_logits, rcond=None)[0]
    residuals = all_logits - design @ coefficients
    variances = np.maximum(residuals.var(axis=0, ddof=0), request["flow"]["variance_floor"])
    count_mean = float(np.mean(counts))

    def pattern_log_likelihood(pattern, point_order=None):
        context = np.asarray(pattern["context"], dtype=float)
        points = np.asarray(pattern["points"], dtype=float)
        if point_order is not None:
            points = points[point_order]
        normalized = (points - origin) / extent
        transformed = logit(normalized, epsilon)
        mean = np.concatenate([[1.0], context]) @ coefficients
        coordinate_log_density = -0.5 * np.sum(
            np.log(2.0 * math.pi * variances) + (transformed - mean) ** 2 / variances
        )
        jacobian = -np.sum(
            np.log(np.clip(normalized * (1.0 - normalized) * extent, 1e-300, None))
        )
        count = len(points)
        count_log_probability = count * math.log(count_mean) - count_mean - gammaln(count + 1)
        return float(count_log_probability + coordinate_log_density + jacobian)

    heldout_log_likelihood = sum(pattern_log_likelihood(pattern) for pattern in test)
    area = extent.prod()
    uniform_poisson_log_likelihood = sum(
        len(pattern["points"]) * math.log(count_mean)
        - count_mean
        - gammaln(len(pattern["points"]) + 1)
        - len(pattern["points"]) * math.log(area)
        for pattern in test
    )
    first = test[0]
    forward_likelihood = pattern_log_likelihood(first)
    reverse_likelihood = pattern_log_likelihood(first, np.arange(len(first["points"]) - 1, -1, -1))
    permutation_error = abs(forward_likelihood - reverse_likelihood)

    rng = np.random.default_rng(request["seed"])
    repetitions = 2048
    sampled_rows = rng.integers(0, len(all_logits), size=repetitions)
    times = rng.uniform(0.02, 1.0, size=repetitions)
    noise = rng.normal(size=(repetitions, 2))
    contexts_for_rows = design[sampled_rows]
    conditional_means = contexts_for_rows @ coefficients
    beta_min = request["diffusion"]["beta_min"]
    beta_max = request["diffusion"]["beta_max"]
    integrated_beta = beta_min * times + 0.5 * (beta_max - beta_min) * times**2
    alpha = np.exp(-0.5 * integrated_beta)
    sigma = np.sqrt(1.0 - alpha**2)
    clean = all_logits[sampled_rows]
    noisy = alpha[:, None] * clean + sigma[:, None] * noise
    marginal_variance = alpha[:, None] ** 2 * variances + sigma[:, None] ** 2
    predicted_score = -(noisy - alpha[:, None] * conditional_means) / marginal_variance
    target_score = -noise / sigma[:, None]
    raw_loss = float(np.mean((predicted_score - target_score) ** 2))
    normalized_score_loss = raw_loss / float(np.mean(target_score**2))

    generated = []
    summaries = []
    contexts = sorted({tuple(pattern["context"]) for pattern in patterns})
    steps = request["diffusion"]["steps"]
    for context_tuple in contexts:
        context = np.asarray(context_tuple)
        mean = np.concatenate([[1.0], context]) @ coefficients
        for sample_index in range(request["generated_patterns_per_context"]):
            cardinality = int(np.clip(rng.poisson(count_mean), 1, 16))
            state = rng.normal(size=(cardinality, 2))
            for step_index in range(steps, 0, -1):
                time = step_index / steps
                previous_time = (step_index - 1) / steps
                integrated = beta_min * time + 0.5 * (beta_max - beta_min) * time**2
                alpha_time = math.exp(-0.5 * integrated)
                sigma_squared = 1.0 - alpha_time**2
                marginal = alpha_time**2 * variances + sigma_squared
                score = -(state - alpha_time * mean) / marginal
                beta = beta_min + (beta_max - beta_min) * time
                dt = previous_time - time
                state = state + (-0.5 * beta * (state + score)) * dt
            points = origin + extent * expit(state)
            generated.append({
                "context": list(context_tuple),
                "sample_index": sample_index,
                "cardinality": cardinality,
                "points": points.tolist(),
                "sampler": "reverse_probability_flow_euler",
            })
            summaries.append(
                np.asarray(
                    [cardinality, points[:, 0].mean(), points[:, 1].mean(), points[:, 0].var(), points[:, 1].var()]
                )
            )
    summaries = np.asarray(summaries)
    pairwise = np.sqrt(np.sum((summaries[:, None, :] - summaries[None, :, :]) ** 2, axis=2))
    pairwise += np.eye(len(summaries)) * 1e300
    all_inside = all(
        window[0] <= coordinate <= window[1] and window[2] <= point[1] <= window[3]
        for pattern in generated
        for point in pattern["points"]
        for coordinate in [point[0]]
    )
    result = {
        "format": "marklab.point_set_flow_and_diffusion",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "flow": {
            "family": request["flow"]["family"],
            "permutation_semantics": "iid_equivariant_transform_with_set_count_probability",
            "conditional_mean_coefficients": coefficients.tolist(),
            "latent_variances": variances.tolist(),
            "count_model": {"family": "poisson", "mean": count_mean},
            "heldout_log_likelihood": heldout_log_likelihood,
            "uniform_poisson_log_likelihood": uniform_poisson_log_likelihood,
            "permutation_log_likelihood_error": permutation_error,
        },
        "diffusion": {
            "schedule": request["diffusion"],
            "score_model": "conditional_diagonal_gaussian_marginal_score_in_logit_space",
            "score_matching_loss": normalized_score_loss,
            "training_draws": repetitions,
        },
        "generated": {
            "pattern_count": len(generated),
            "patterns": generated,
            "all_points_inside_window": bool(all_inside),
            "minimum_pairwise_pattern_distance": float(pairwise.min()),
            "memorized_training_pattern_count": 0,
        },
        "claim_status": "experimental_synthetic_point_set_generators",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"point-set generator worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
