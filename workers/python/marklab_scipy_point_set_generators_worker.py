#!/usr/bin/env python3
"""SciPy worker for a conditional point-set flow and a matching diffusion sampler.

Points are mapped into the unit square and through a clipped logit; in that space each point is
an independent diagonal Gaussian whose mean is linear in the pattern context (the flow), and the
pattern size is Poisson. The diffusion part trains nothing: it evaluates the closed-form score
of that Gaussian under a variance-preserving schedule and samples new patterns with a reverse
probability-flow Euler integrator.
"""

import json
import math
import sys
from pathlib import Path
from typing import Any

import numpy as np
import scipy
from scipy.special import expit, gammaln

sys.path.append(str(Path(__file__).resolve().parent))

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

SCORE_MATCHING_DRAWS = 2048
MAXIMUM_GENERATED_POINTS = 16


def logit(values: np.ndarray, epsilon: float) -> np.ndarray:
    """Logit of ``values`` clipped to ``[epsilon, 1 - epsilon]``."""
    clipped = np.clip(values, epsilon, 1.0 - epsilon)
    return np.log(clipped) - np.log1p(-clipped)


class Flow:
    """Context-conditional diagonal Gaussian in logit space with a Poisson pattern size."""

    def __init__(self, request: dict[str, Any], train: list[dict[str, Any]]) -> None:
        window = np.asarray(request["patterns"][0]["window"], dtype=float)
        self.window = window
        self.origin = np.asarray([window[0], window[2]])
        self.extent = np.asarray([window[1] - window[0], window[3] - window[2]])
        self.epsilon = request["flow"]["boundary_epsilon"]
        design_rows = []
        logits = []
        counts = []
        for pattern in train:
            context = np.asarray(pattern["context"], dtype=float)
            transformed = self.transform(np.asarray(pattern["points"], dtype=float))
            logits.append(transformed)
            context_rows = np.broadcast_to(context, (len(transformed), len(context)))
            design_rows.append(np.column_stack([np.ones(len(transformed)), context_rows]))
            counts.append(len(transformed))
        self.logits = np.concatenate(logits, axis=0)
        self.design = np.concatenate(design_rows, axis=0)
        self.coefficients = np.linalg.lstsq(self.design, self.logits, rcond=None)[0]
        residuals = self.logits - self.design @ self.coefficients
        self.variances = np.maximum(
            residuals.var(axis=0, ddof=0), request["flow"]["variance_floor"]
        )
        self.count_mean = float(np.mean(counts))

    def normalized(self, points: np.ndarray) -> np.ndarray:
        return (points - self.origin) / self.extent

    def transform(self, points: np.ndarray) -> np.ndarray:
        return logit(self.normalized(points), self.epsilon)

    def mean(self, context: Any) -> np.ndarray:
        return np.concatenate([[1.0], context]) @ self.coefficients

    def count_log_probability(self, count: int) -> float:
        return count * math.log(self.count_mean) - self.count_mean - gammaln(count + 1)

    def log_likelihood(self, pattern: dict[str, Any], point_order: Any = None) -> float:
        """Pattern log likelihood: Poisson count, Gaussian logits, and the logit Jacobian."""
        context = np.asarray(pattern["context"], dtype=float)
        points = np.asarray(pattern["points"], dtype=float)
        if point_order is not None:
            points = points[point_order]
        normalized = self.normalized(points)
        transformed = logit(normalized, self.epsilon)
        mean = self.mean(context)
        coordinate_log_density = -0.5 * np.sum(
            np.log(2.0 * math.pi * self.variances) + (transformed - mean) ** 2 / self.variances
        )
        jacobian = -np.sum(
            np.log(np.clip(normalized * (1.0 - normalized) * self.extent, 1e-300, None))
        )
        count_log_probability = self.count_log_probability(len(points))
        return float(count_log_probability + coordinate_log_density + jacobian)


def integrated_beta(schedule: dict[str, Any], time: Any) -> Any:
    """``int_0^t beta(s) ds`` for the linear schedule ``beta(s) = beta_min + (beta_max -
    beta_min) * s``."""
    beta_min = schedule["beta_min"]
    beta_max = schedule["beta_max"]
    return beta_min * time + 0.5 * (beta_max - beta_min) * time**2


def score_matching_loss(flow: Flow, schedule: dict[str, Any], rng: np.random.Generator) -> float:
    """Denoising score-matching loss of the closed-form score, normalized by the target scale."""
    sampled_rows = rng.integers(0, len(flow.logits), size=SCORE_MATCHING_DRAWS)
    times = rng.uniform(0.02, 1.0, size=SCORE_MATCHING_DRAWS)
    noise = rng.normal(size=(SCORE_MATCHING_DRAWS, 2))
    conditional_means = flow.design[sampled_rows] @ flow.coefficients
    alpha = np.exp(-0.5 * integrated_beta(schedule, times))
    sigma = np.sqrt(1.0 - alpha**2)
    clean = flow.logits[sampled_rows]
    noisy = alpha[:, None] * clean + sigma[:, None] * noise
    marginal_variance = alpha[:, None] ** 2 * flow.variances + sigma[:, None] ** 2
    predicted_score = -(noisy - alpha[:, None] * conditional_means) / marginal_variance
    target_score = -noise / sigma[:, None]
    raw_loss = float(np.mean((predicted_score - target_score) ** 2))
    return raw_loss / float(np.mean(target_score**2))


def generate(
    flow: Flow, request: dict[str, Any], rng: np.random.Generator
) -> tuple[list[dict[str, Any]], np.ndarray]:
    """Sample patterns per context by reverse probability-flow Euler steps from noise.

    Returns the patterns and, per pattern, its size and coordinate means and variances.
    """
    schedule = request["diffusion"]
    beta_min = schedule["beta_min"]
    beta_max = schedule["beta_max"]
    steps = schedule["steps"]
    generated = []
    summaries = []
    contexts = sorted({tuple(pattern["context"]) for pattern in request["patterns"]})
    for context_tuple in contexts:
        mean = flow.mean(np.asarray(context_tuple))
        for sample_index in range(request["generated_patterns_per_context"]):
            cardinality = int(np.clip(rng.poisson(flow.count_mean), 1, MAXIMUM_GENERATED_POINTS))
            state = rng.normal(size=(cardinality, 2))
            for step_index in range(steps, 0, -1):
                time = step_index / steps
                previous_time = (step_index - 1) / steps
                alpha_time = math.exp(-0.5 * integrated_beta(schedule, time))
                sigma_squared = 1.0 - alpha_time**2
                marginal = alpha_time**2 * flow.variances + sigma_squared
                score = -(state - alpha_time * mean) / marginal
                beta = beta_min + (beta_max - beta_min) * time
                dt = previous_time - time
                state = state + (-0.5 * beta * (state + score)) * dt
            points = flow.origin + flow.extent * expit(state)
            generated.append(
                {
                    "context": list(context_tuple),
                    "sample_index": sample_index,
                    "cardinality": cardinality,
                    "points": points.tolist(),
                    "sampler": "reverse_probability_flow_euler",
                }
            )
            summaries.append(
                np.asarray(
                    [
                        cardinality,
                        points[:, 0].mean(),
                        points[:, 1].mean(),
                        points[:, 0].var(),
                        points[:, 1].var(),
                    ]
                )
            )
    return generated, np.asarray(summaries)


def main() -> None:
    require_versions(numpy=np.__version__, scipy=scipy.__version__)
    request_bytes = read_request(32 * MIB)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.scipy_point_set_generators_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    patterns = request["patterns"]
    train = [pattern for pattern in patterns if pattern["split"] == "train"]
    test = [pattern for pattern in patterns if pattern["split"] == "test"]
    flow = Flow(request, train)
    heldout_log_likelihood = sum(flow.log_likelihood(pattern) for pattern in test)
    area = flow.extent.prod()
    uniform_poisson_log_likelihood = sum(
        flow.count_log_probability(len(pattern["points"])) - len(pattern["points"]) * math.log(area)
        for pattern in test
    )
    first = test[0]
    forward_likelihood = flow.log_likelihood(first)
    reversed_order = np.arange(len(first["points"]) - 1, -1, -1)
    reverse_likelihood = flow.log_likelihood(first, reversed_order)
    permutation_error = abs(forward_likelihood - reverse_likelihood)
    rng = np.random.default_rng(request["seed"])
    normalized_score_loss = score_matching_loss(flow, request["diffusion"], rng)
    generated, summaries = generate(flow, request, rng)
    pairwise = np.sqrt(np.sum((summaries[:, None, :] - summaries[None, :, :]) ** 2, axis=2))
    pairwise += np.eye(len(summaries)) * 1e300
    window = flow.window
    all_inside = all(
        window[0] <= point[0] <= window[1] and window[2] <= point[1] <= window[3]
        for pattern in generated
        for point in pattern["points"]
    )
    result = {
        "format": "marklab.point_set_flow_and_diffusion",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "flow": {
            "family": request["flow"]["family"],
            "permutation_semantics": "iid_equivariant_transform_with_set_count_probability",
            "conditional_mean_coefficients": flow.coefficients.tolist(),
            "latent_variances": flow.variances.tolist(),
            "count_model": {"family": "poisson", "mean": flow.count_mean},
            "heldout_log_likelihood": heldout_log_likelihood,
            "uniform_poisson_log_likelihood": uniform_poisson_log_likelihood,
            "permutation_log_likelihood_error": permutation_error,
        },
        "diffusion": {
            "schedule": request["diffusion"],
            "score_model": "conditional_diagonal_gaussian_marginal_score_in_logit_space",
            "score_matching_loss": normalized_score_loss,
            "training_draws": SCORE_MATCHING_DRAWS,
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
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "point-set generator worker contract error", errors=CONTRACT_ERRORS)
