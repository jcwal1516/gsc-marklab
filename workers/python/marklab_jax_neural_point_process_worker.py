#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import jax
import jax.numpy as jnp
import numpy as np
import scipy
from scipy.optimize import minimize


class ContractError(Exception):
    pass


def main():
    if (
        jax.__version__ != "0.11.1"
        or np.__version__ != "2.4.6"
        or scipy.__version__ != "1.18.1"
        or sys.version_info[:2] != (3, 12)
    ):
        raise ContractError("neural point-process backend version drift")
    jax.config.update("jax_enable_x64", True)
    request_bytes = sys.stdin.buffer.read(32 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.jax_neural_point_process_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    patterns = request["patterns"]
    train_patterns = [pattern for pattern in patterns if pattern["split"] == "train"]
    test_patterns = [pattern for pattern in patterns if pattern["split"] == "test"]
    mark_labels = request["mark_labels"]
    mark_index = {label: index for index, label in enumerate(mark_labels)}
    context_dimension = len(patterns[0]["context"])
    input_dimension = 2 + context_dimension
    hidden = request["architecture"]["hidden_units"]
    mark_count = len(mark_labels)
    shapes = [
        (input_dimension, hidden),
        (hidden,),
        (hidden,),
        (1,),
        (hidden, mark_count),
        (mark_count,),
    ]
    random = np.random.default_rng(request["seed"])
    initial_parts = [
        random.normal(0.0, 0.2, size=shapes[0]),
        np.zeros(shapes[1]),
        random.normal(0.0, 0.2, size=shapes[2]),
        np.asarray([math.log(math.expm1(20.0))]),
        random.normal(0.0, 0.2, size=shapes[4]),
        np.zeros(shapes[5]),
    ]
    initial = np.concatenate([part.ravel() for part in initial_parts])

    def unpack(parameters):
        offset = 0
        parts = []
        for shape in shapes:
            size = int(np.prod(shape))
            parts.append(parameters[offset : offset + size].reshape(shape))
            offset += size
        return parts

    def network(parameters, locations, context):
        weight, bias, intensity_weight, intensity_bias, mark_weight, mark_bias = unpack(parameters)
        repeated_context = jnp.broadcast_to(context, (locations.shape[0], context.shape[0]))
        inputs = jnp.concatenate([locations, repeated_context], axis=1)
        representation = jnp.tanh(inputs @ weight + bias)
        intensity = jax.nn.softplus(representation @ intensity_weight + intensity_bias[0]) + 1e-8
        mark_log_probability = jax.nn.log_softmax(representation @ mark_weight + mark_bias, axis=1)
        return intensity, mark_log_probability

    def quadrature_locations(window, grid_size):
        x0, x1, y0, y1 = window
        xs = jnp.linspace(x0 + (x1 - x0) / (2 * grid_size), x1 - (x1 - x0) / (2 * grid_size), grid_size)
        ys = jnp.linspace(y0 + (y1 - y0) / (2 * grid_size), y1 - (y1 - y0) / (2 * grid_size), grid_size)
        yy, xx = jnp.meshgrid(ys, xs, indexing="ij")
        return jnp.stack([xx.ravel(), yy.ravel()], axis=1)

    prepared_train = []
    for pattern in train_patterns:
        locations = jnp.asarray([point["coordinates"] for point in pattern["points"]])
        marks = jnp.asarray([mark_index[point["mark"]] for point in pattern["points"]], dtype=int)
        context = jnp.asarray(pattern["context"])
        quadrature = quadrature_locations(pattern["window"], request["quadrature_grid"])
        area = (pattern["window"][1] - pattern["window"][0]) * (pattern["window"][3] - pattern["window"][2])
        prepared_train.append((locations, marks, context, quadrature, area))

    def log_likelihood(parameters, prepared):
        total = 0.0
        for locations, marks, context, quadrature, area in prepared:
            intensity, mark_log_probability = network(parameters, locations, context)
            quadrature_intensity, _ = network(parameters, quadrature, context)
            total += jnp.sum(jnp.log(intensity)) - area * jnp.mean(quadrature_intensity)
            total += jnp.sum(mark_log_probability[jnp.arange(len(marks)), marks])
        return total

    def objective(parameters):
        return -log_likelihood(parameters, prepared_train) + 0.5 * request["parameter_precision"] * jnp.sum(parameters * parameters)

    value_and_gradient = jax.jit(jax.value_and_grad(objective))

    def scipy_objective(parameters):
        value, gradient = value_and_gradient(jnp.asarray(parameters))
        return float(value), np.asarray(gradient, dtype=float)

    initial_objective = scipy_objective(initial)[0]
    fit = minimize(
        scipy_objective,
        initial,
        jac=True,
        method="L-BFGS-B",
        options={"maxiter": request["maximum_iterations"], "ftol": 1e-8, "gtol": 1e-4},
    )
    if not fit.success or not np.isfinite(fit.x).all():
        raise ContractError(f"neural point-process training did not converge: {fit.message}")
    prepared_test = []
    mark_correct = 0
    mark_total = 0
    neural_log_likelihood = 0.0
    integral_coarse = []
    integral_reference = []
    for pattern in test_patterns:
        locations = jnp.asarray([point["coordinates"] for point in pattern["points"]])
        marks_np = np.asarray([mark_index[point["mark"]] for point in pattern["points"]], dtype=int)
        marks = jnp.asarray(marks_np)
        context = jnp.asarray(pattern["context"])
        coarse = quadrature_locations(pattern["window"], request["quadrature_grid"])
        reference = quadrature_locations(pattern["window"], request["quadrature_reference_grid"])
        area = (pattern["window"][1] - pattern["window"][0]) * (pattern["window"][3] - pattern["window"][2])
        prepared_test.append((locations, marks, context, coarse, area))
        intensity, mark_logs = network(jnp.asarray(fit.x), locations, context)
        coarse_intensity, _ = network(jnp.asarray(fit.x), coarse, context)
        reference_intensity, _ = network(jnp.asarray(fit.x), reference, context)
        neural_log_likelihood += float(jnp.sum(jnp.log(intensity)) - area * jnp.mean(coarse_intensity))
        neural_log_likelihood += float(jnp.sum(mark_logs[jnp.arange(len(marks)), marks]))
        mark_correct += int(np.sum(np.asarray(jnp.argmax(mark_logs, axis=1)) == marks_np))
        mark_total += len(marks_np)
        integral_coarse.append(float(area * jnp.mean(coarse_intensity)))
        integral_reference.append(float(area * jnp.mean(reference_intensity)))
    training_point_count = sum(len(pattern["points"]) for pattern in train_patterns)
    training_area = sum(
        (pattern["window"][1] - pattern["window"][0]) * (pattern["window"][3] - pattern["window"][2])
        for pattern in train_patterns
    )
    homogeneous_intensity = training_point_count / training_area
    mark_counts = np.zeros(mark_count)
    for pattern in train_patterns:
        for point in pattern["points"]:
            mark_counts[mark_index[point["mark"]]] += 1
    mark_probability = mark_counts / mark_counts.sum()
    homogeneous_log_likelihood = 0.0
    for pattern in test_patterns:
        area = (pattern["window"][1] - pattern["window"][0]) * (pattern["window"][3] - pattern["window"][2])
        homogeneous_log_likelihood += len(pattern["points"]) * math.log(homogeneous_intensity) - area * homogeneous_intensity
        homogeneous_log_likelihood += sum(math.log(mark_probability[mark_index[point["mark"]]]) for point in pattern["points"])
    relative_integral_difference = max(
        abs(coarse - reference) / max(abs(reference), 1e-12)
        for coarse, reference in zip(integral_coarse, integral_reference)
    )
    probe_context = jnp.asarray([1.0])
    probe_locations = jnp.asarray([[0.2, 0.5], [0.8, 0.5]])
    probe_intensity, _ = network(jnp.asarray(fit.x), probe_locations, probe_context)
    weights = [np.asarray(part).tolist() for part in unpack(fit.x)]
    result = {
        "format": "marklab.neural_marked_cox_process",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "architecture": request["architecture"],
        "network_parameters": {
            "input_hidden_weight": weights[0],
            "hidden_bias": weights[1],
            "intensity_weight": weights[2],
            "intensity_bias": weights[3],
            "mark_weight": weights[4],
            "mark_bias": weights[5],
        },
        "heldout": {
            "pattern_count": len(test_patterns),
            "joint_log_likelihood": neural_log_likelihood,
            "homogeneous_independent_mark_log_likelihood": homogeneous_log_likelihood,
            "mark_accuracy": mark_correct / mark_total,
        },
        "quadrature": {
            "training_grid": request["quadrature_grid"],
            "reference_grid": request["quadrature_reference_grid"],
            "heldout_integrals": integral_coarse,
            "heldout_reference_integrals": integral_reference,
            "relative_integral_difference": relative_integral_difference,
        },
        "probe_intensities": {
            "context_positive_low_x": float(probe_intensity[0]),
            "context_positive_high_x": float(probe_intensity[1]),
        },
        "diagnostics": {
            "objective_initial": initial_objective,
            "objective_final": float(fit.fun),
            "iterations": int(fit.nit),
            "gradient_max_absolute": float(np.max(np.abs(fit.jac))),
            "deterministic_cpu": True,
        },
        "fit_state": "complete",
        "claim_status": "experimental_synthetic_neural_marked_cox_process",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"neural point-process worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
