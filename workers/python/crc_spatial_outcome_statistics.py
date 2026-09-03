#!/usr/bin/env python3
"""Patient-unit statistics for the CRC spatial phenotype outcome analysis."""

from __future__ import annotations

from collections import defaultdict
from itertools import combinations
import math
import random
import statistics
from typing import Any, Dict, Iterable, List, Sequence, Tuple


def _finite(value: Any, label: str) -> float:
    parsed = float(value)
    if not math.isfinite(parsed):
        raise ValueError(f"{label} must be finite")
    return parsed


def _percentile(values: Sequence[float], probability: float) -> float:
    if not values or not 0.0 <= probability <= 1.0:
        raise ValueError("percentile requires finite values and a probability in [0,1]")
    ordered = sorted(_finite(value, "percentile value") for value in values)
    position = (len(ordered) - 1) * probability
    lower = int(math.floor(position))
    upper = int(math.ceil(position))
    if lower == upper:
        return ordered[lower]
    fraction = position - lower
    return ordered[lower] * (1.0 - fraction) + ordered[upper] * fraction


def _median(values: Iterable[float]) -> float:
    materialized = [_finite(value, "median value") for value in values]
    if not materialized:
        raise ValueError("median requires at least one finite value")
    return float(statistics.median(materialized))


def _mean(values: Sequence[float]) -> float:
    if not values:
        raise ValueError("mean requires at least one value")
    result = math.fsum(_finite(value, "mean value") for value in values) / len(values)
    if not math.isfinite(result):
        raise ValueError("mean is non-finite")
    return result


def benjamini_hochberg(p_values: Sequence[float]) -> List[float]:
    """Return BH q-values in the caller's original order."""
    parsed = [_finite(value, "p-value") for value in p_values]
    if any(value < 0.0 or value > 1.0 for value in parsed):
        raise ValueError("p-values must lie in [0,1]")
    count = len(parsed)
    if count == 0:
        return []
    order = sorted(range(count), key=lambda index: (parsed[index], index))
    result = [0.0] * count
    running = 1.0
    for rank_index in range(count - 1, -1, -1):
        index = order[rank_index]
        rank = rank_index + 1
        running = min(running, parsed[index] * count / rank)
        result[index] = min(1.0, running)
    return result


def patient_label_permutation(
    records: Sequence[Dict[str, Any]],
    group_a: str,
    group_b: str,
    permutations: int,
    seed: int,
) -> Dict[str, Any]:
    """Compare one scalar per patient by whole-patient label permutation."""
    if not group_a or not group_b or group_a == group_b:
        raise ValueError("two distinct nonempty groups are required")
    if permutations < 1:
        raise ValueError("permutations must be positive")
    patients = set()
    values: List[float] = []
    labels: List[str] = []
    for row in records:
        patient = str(row.get("patient_id", ""))
        if not patient or patient.strip() != patient:
            raise ValueError("patient_id must be nonempty without surrounding whitespace")
        if patient in patients:
            raise ValueError(f"duplicate patient: {patient}")
        patients.add(patient)
        group = str(row.get("group", ""))
        if group not in {group_a, group_b}:
            raise ValueError(f"patient {patient} has an undeclared group")
        labels.append(group)
        values.append(_finite(row.get("value"), "patient value"))
    count_a = sum(label == group_a for label in labels)
    count_b = len(labels) - count_a
    if min(count_a, count_b) < 2:
        raise ValueError("each group requires at least two independent patients")

    def statistic(indices_a: Sequence[int]) -> float:
        selected = set(indices_a)
        left = [value for index, value in enumerate(values) if index in selected]
        right = [value for index, value in enumerate(values) if index not in selected]
        return _mean(left) - _mean(right)

    observed_indices = [index for index, label in enumerate(labels) if label == group_a]
    observed = statistic(observed_indices)
    combination_count = math.comb(len(values), count_a)
    if combination_count <= permutations + 1:
        null = [statistic(indices) for indices in combinations(range(len(values)), count_a)]
        extreme = sum(abs(value) >= abs(observed) - 1e-15 for value in null)
        p_value = extreme / len(null)
        mode = "exact_all_whole_patient_label_assignments"
        completed = len(null)
    else:
        generator = random.Random(seed)
        indices = list(range(len(values)))
        null = []
        for _ in range(permutations):
            generator.shuffle(indices)
            null.append(statistic(indices[:count_a]))
        extreme = sum(abs(value) >= abs(observed) - 1e-15 for value in null)
        p_value = (extreme + 1) / (permutations + 1)
        mode = "monte_carlo_whole_patient_label_permutation"
        completed = permutations
    return {
        "population_unit": "patient",
        "permutation_unit": "whole_patient_label",
        "mode": mode,
        "alternative": "two_sided",
        "group_counts": {group_a: count_a, group_b: count_b},
        "difference_in_means": observed,
        "group_medians": {
            group_a: _median(
                value for value, label in zip(values, labels) if label == group_a
            ),
            group_b: _median(
                value for value, label in zip(values, labels) if label == group_b
            ),
        },
        "p_value": p_value,
        "permutations_requested": permutations,
        "permutations_completed": completed,
        "seed": seed,
    }


def _rms_distance(left: Sequence[float], right: Sequence[float]) -> float:
    if len(left) != len(right) or not left:
        raise ValueError("distance vectors require one common nonempty width")
    value = math.sqrt(
        math.fsum((a - b) * (a - b) for a, b in zip(left, right)) / len(left)
    )
    if not math.isfinite(value):
        raise ValueError("specimen distance is non-finite")
    return value


def repeatability_analysis(
    records: Sequence[Dict[str, Any]],
    feature_names: Sequence[str],
    bootstrap_replicates: int,
    seed: int,
) -> Dict[str, Any]:
    """Summarize repeated specimens without promoting them to patient replicates."""
    features = list(feature_names)
    if not features or len(features) != len(set(features)):
        raise ValueError("repeatability features must be nonempty and unique")
    if bootstrap_replicates < 1:
        raise ValueError("bootstrap_replicates must be positive")
    identities = set()
    parsed: List[Tuple[str, str, List[float]]] = []
    columns: List[List[float]] = [[] for _ in features]
    for row in records:
        patient = str(row.get("patient_id", ""))
        specimen = str(row.get("specimen_id", ""))
        if not patient or not specimen or patient.strip() != patient or specimen.strip() != specimen:
            raise ValueError("patient and specimen identities must be exact and nonempty")
        identity = (patient, specimen)
        if identity in identities:
            raise ValueError(f"duplicate patient/specimen row: {patient}/{specimen}")
        identities.add(identity)
        vector = [_finite(row.get(name), name) for name in features]
        for column, value in zip(columns, vector):
            column.append(value)
        parsed.append((patient, specimen, vector))
    if len(parsed) < 4 or len({row[0] for row in parsed}) < 2:
        raise ValueError("repeatability requires at least two patients and four specimens")

    scaling = []
    for name, column in zip(features, columns):
        center = _median(column)
        mad = _median(abs(value - center) for value in column)
        scale = 1.4826 * mad
        basis = "median_absolute_deviation"
        if scale <= 0.0:
            scale = max(column) - min(column)
            basis = "range_fallback"
        if not math.isfinite(scale) or scale <= 0.0:
            raise ValueError(f"repeatability feature is constant: {name}")
        scaling.append({"feature": name, "center": center, "scale": scale, "basis": basis})
    transformed = []
    for patient, specimen, vector in parsed:
        transformed.append(
            (
                patient,
                specimen,
                [
                    (value - parameter["center"]) / parameter["scale"]
                    for value, parameter in zip(vector, scaling)
                ],
            )
        )
    by_patient: Dict[str, List[Tuple[str, List[float]]]] = defaultdict(list)
    for patient, specimen, vector in transformed:
        by_patient[patient].append((specimen, vector))
    repeated = {patient: values for patient, values in by_patient.items() if len(values) >= 2}
    if len(repeated) < 2:
        raise ValueError("repeatability requires at least two patients with repeated specimens")

    per_patient = []
    for patient in sorted(repeated):
        own = repeated[patient]
        within = _median(
            _rms_distance(left[1], right[1])
            for index, left in enumerate(own)
            for right in own[index + 1 :]
        )
        between = _median(
            _rms_distance(left[1], right[1])
            for left in own
            for other_patient, other in by_patient.items()
            if other_patient != patient
            for right in other
        )
        retrieval = []
        for query_specimen, query in own:
            candidates = []
            for candidate_patient, candidate_rows in by_patient.items():
                candidate_vectors = [
                    vector
                    for specimen, vector in candidate_rows
                    if candidate_patient != patient or specimen != query_specimen
                ]
                if not candidate_vectors:
                    continue
                centroid = [
                    math.fsum(vector[index] for vector in candidate_vectors)
                    / len(candidate_vectors)
                    for index in range(len(features))
                ]
                candidates.append(
                    (_rms_distance(query, centroid), candidate_patient)
                )
            nearest = min(candidates, key=lambda item: (item[0], item[1]))[1]
            retrieval.append(1.0 if nearest == patient else 0.0)
        per_patient.append(
            {
                "patient_id": patient,
                "specimen_count": len(own),
                "within_patient_distance": within,
                "between_patient_distance": between,
                "between_minus_within_distance": between - within,
                "retrieval_accuracy": _mean(retrieval),
            }
        )
    within_values = [row["within_patient_distance"] for row in per_patient]
    between_values = [row["between_patient_distance"] for row in per_patient]
    effects = [row["between_minus_within_distance"] for row in per_patient]
    retrieval_values = [row["retrieval_accuracy"] for row in per_patient]
    generator = random.Random(seed)
    bootstrap_effect = []
    bootstrap_retrieval = []
    for _ in range(bootstrap_replicates):
        sampled = [per_patient[generator.randrange(len(per_patient))] for _ in per_patient]
        bootstrap_effect.append(_median(row["between_minus_within_distance"] for row in sampled))
        bootstrap_retrieval.append(_mean([row["retrieval_accuracy"] for row in sampled]))
    return {
        "population_unit": "patient",
        "specimen_role": "repeated_measurement_not_population_replicate",
        "patient_count": len(by_patient),
        "repeated_patient_count": len(per_patient),
        "specimen_count": len(parsed),
        "feature_count": len(features),
        "features": features,
        "distance": "root_mean_squared_robust_scaled_feature_distance",
        "scaling": scaling,
        "median_within_patient_distance": _median(within_values),
        "median_between_patient_distance": _median(between_values),
        "median_between_minus_within_distance": _median(effects),
        "leave_one_specimen_out_patient_retrieval": _mean(retrieval_values),
        "per_patient": per_patient,
        "bootstrap": {
            "resampling_unit": "whole_patient",
            "replicates": bootstrap_replicates,
            "seed": seed,
            "between_minus_within_interval_95": [
                _percentile(bootstrap_effect, 0.025),
                _percentile(bootstrap_effect, 0.975),
            ],
            "retrieval_interval_95": [
                _percentile(bootstrap_retrieval, 0.025),
                _percentile(bootstrap_retrieval, 0.975),
            ],
        },
    }


def _solve(matrix: Sequence[Sequence[float]], vector: Sequence[float]) -> List[float]:
    size = len(vector)
    if size == 0 or len(matrix) != size or any(len(row) != size for row in matrix):
        raise ValueError("linear system dimensions differ")
    augmented = [
        [_finite(value, "matrix value") for value in row]
        + [_finite(vector[index], "vector value")]
        for index, row in enumerate(matrix)
    ]
    for column in range(size):
        pivot = max(range(column, size), key=lambda row: abs(augmented[row][column]))
        if abs(augmented[pivot][column]) < 1e-12:
            raise ValueError("Cox information matrix is singular")
        augmented[column], augmented[pivot] = augmented[pivot], augmented[column]
        divisor = augmented[column][column]
        augmented[column] = [value / divisor for value in augmented[column]]
        for row in range(size):
            if row == column:
                continue
            factor = augmented[row][column]
            if factor == 0.0:
                continue
            augmented[row] = [
                value - factor * pivot_value
                for value, pivot_value in zip(augmented[row], augmented[column])
            ]
    return [row[-1] for row in augmented]


def _inverse(matrix: Sequence[Sequence[float]]) -> List[List[float]]:
    size = len(matrix)
    columns = []
    for column in range(size):
        basis = [0.0] * size
        basis[column] = 1.0
        columns.append(_solve(matrix, basis))
    return [[columns[column][row] for column in range(size)] for row in range(size)]


def fit_cox_breslow(
    times: Sequence[float],
    events: Sequence[int],
    design_matrix: Sequence[Sequence[float]],
    coefficient_names: Sequence[str],
    maximum_iterations: int = 80,
) -> Dict[str, Any]:
    """Fit a bounded patient-level Cox model using Breslow ties."""
    count = len(times)
    width = len(coefficient_names)
    if (
        count < 6
        or len(events) != count
        or len(design_matrix) != count
        or width < 1
        or width != len(set(coefficient_names))
        or any(len(row) != width for row in design_matrix)
    ):
        raise ValueError("Cox input dimensions or coefficient names are invalid")
    parsed_times = [_finite(value, "survival time") for value in times]
    if any(value <= 0.0 for value in parsed_times):
        raise ValueError("survival times must be positive")
    parsed_events = [int(value) for value in events]
    if any(value not in {0, 1} for value in parsed_events) or sum(parsed_events) < 2:
        raise ValueError("Cox events must be binary with at least two events")
    matrix = [[_finite(value, "Cox covariate") for value in row] for row in design_matrix]
    centers = []
    scales = []
    for column in range(width):
        values = [row[column] for row in matrix]
        center = _mean(values)
        variance = math.fsum((value - center) ** 2 for value in values) / (count - 1)
        scale = math.sqrt(variance)
        if not math.isfinite(scale) or scale <= 0.0:
            raise ValueError(f"Cox covariate is constant: {coefficient_names[column]}")
        centers.append(center)
        scales.append(scale)
    standardized = [
        [(value - centers[column]) / scales[column] for column, value in enumerate(row)]
        for row in matrix
    ]
    event_times = sorted({time for time, event in zip(parsed_times, parsed_events) if event})

    def state(beta: Sequence[float]) -> Tuple[float, List[float], List[List[float]]]:
        eta = [math.fsum(value * coefficient for value, coefficient in zip(row, beta)) for row in standardized]
        log_likelihood = 0.0
        score = [0.0] * width
        information = [[0.0] * width for _ in range(width)]
        for event_time in event_times:
            deaths = [index for index, (time, event) in enumerate(zip(parsed_times, parsed_events)) if event and time == event_time]
            risk = [index for index, time in enumerate(parsed_times) if time >= event_time]
            maximum_eta = max(eta[index] for index in risk)
            weights = [math.exp(eta[index] - maximum_eta) for index in risk]
            total_weight = math.fsum(weights)
            mean = [
                math.fsum(weight * standardized[index][column] for weight, index in zip(weights, risk)) / total_weight
                for column in range(width)
            ]
            second = [[0.0] * width for _ in range(width)]
            for weight, index in zip(weights, risk):
                row = standardized[index]
                for left in range(width):
                    for right in range(width):
                        second[left][right] += weight * row[left] * row[right] / total_weight
            death_count = len(deaths)
            log_likelihood += math.fsum(eta[index] for index in deaths)
            log_likelihood -= death_count * (maximum_eta + math.log(total_weight))
            for column in range(width):
                score[column] += math.fsum(standardized[index][column] for index in deaths)
                score[column] -= death_count * mean[column]
            for left in range(width):
                for right in range(width):
                    information[left][right] += death_count * (
                        second[left][right] - mean[left] * mean[right]
                    )
        return log_likelihood, score, information

    beta = [0.0] * width
    converged = False
    iterations = 0
    for iteration in range(1, maximum_iterations + 1):
        iterations = iteration
        likelihood, score, information = state(beta)
        for index in range(width):
            information[index][index] += 1e-10
        step = _solve(information, score)
        if max(abs(value) for value in step) < 1e-8:
            converged = True
            break
        fraction = 1.0
        accepted = False
        while fraction >= 2.0 ** -20:
            candidate = [value + fraction * delta for value, delta in zip(beta, step)]
            candidate_likelihood, _, _ = state(candidate)
            if candidate_likelihood >= likelihood - 1e-12:
                beta = candidate
                accepted = True
                break
            fraction *= 0.5
        if not accepted:
            break
        if max(abs(fraction * value) for value in step) < 1e-8:
            converged = True
            break
    likelihood, score, information = state(beta)
    for index in range(width):
        information[index][index] += 1e-10
    covariance = _inverse(information)
    effects = []
    for index, name in enumerate(coefficient_names):
        standard_error = math.sqrt(covariance[index][index])
        coefficient = beta[index]
        z_value = coefficient / standard_error
        lower = coefficient - 1.959963984540054 * standard_error
        upper = coefficient + 1.959963984540054 * standard_error
        effects.append(
            {
                "name": name,
                "unit": "one_cohort_standard_deviation",
                "coefficient": coefficient,
                "standard_error": standard_error,
                "z_value": z_value,
                "p_value": math.erfc(abs(z_value) / math.sqrt(2.0)),
                "hazard_ratio": math.exp(coefficient),
                "hazard_ratio_ci_95": [math.exp(lower), math.exp(upper)],
                "source_center": centers[index],
                "source_scale": scales[index],
            }
        )
    if not all(
        math.isfinite(value)
        for effect in effects
        for value in (
            effect["coefficient"],
            effect["standard_error"],
            effect["hazard_ratio"],
            *effect["hazard_ratio_ci_95"],
        )
    ):
        raise ValueError("Cox fit produced a non-finite effect")
    return {
        "population_unit": "patient",
        "model": "cox_proportional_hazards",
        "ties": "breslow",
        "patient_count": count,
        "event_count": sum(parsed_events),
        "censored_count": count - sum(parsed_events),
        "converged": converged,
        "iterations": iterations,
        "maximum_absolute_score": max(abs(value) for value in score),
        "log_partial_likelihood": likelihood,
        "effects": effects,
    }
