#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import numpy as np
import scipy


class ContractError(Exception):
    pass


def fit_atlas(samples, domains, ridge):
    features = np.asarray([sample["features"] for sample in samples], dtype=float)
    prototypes = {
        domain: features[[sample["domain"] == domain for sample in samples]].mean(axis=0)
        for domain in domains
    }
    residuals = np.asarray(
        [np.asarray(sample["features"]) - prototypes[sample["domain"]] for sample in samples]
    )
    covariance = np.cov(residuals, rowvar=False, bias=True)
    covariance = np.atleast_2d(covariance) + ridge * np.eye(features.shape[1])
    inverse = np.linalg.inv(covariance)
    return prototypes, covariance, inverse


def distances(features, domains, prototypes, inverse):
    return np.asarray(
        [
            float((features - prototypes[domain]) @ inverse @ (features - prototypes[domain]))
            for domain in domains
        ]
    )


def probabilities_from_distances(squared_distances):
    logits = -0.5 * squared_distances
    logits -= logits.max()
    scores = np.exp(logits)
    return scores / scores.sum()


def main():
    if np.__version__ != "2.4.6" or scipy.__version__ != "1.18.1" or sys.version_info[:2] != (3, 12):
        raise ContractError("atlas backend version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.scipy_atlas_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    samples = request["reference_samples"]
    domains = sorted({sample["domain"] for sample in samples})
    patients = sorted({sample["patient_id"] for sample in samples})
    sites = sorted({sample["site_id"] for sample in samples})
    feature_count = len(request["representation"]["feature_names"])
    prototypes, covariance, inverse = fit_atlas(samples, domains, request["covariance_ridge"])
    patient_domain_means = {}
    for domain in domains:
        means = []
        for patient in patients:
            rows = [sample["features"] for sample in samples if sample["patient_id"] == patient and sample["domain"] == domain]
            if rows:
                means.append(np.mean(rows, axis=0))
        patient_domain_means[domain] = np.asarray(means, dtype=float)
    prototype_payload = []
    for domain in domains:
        between = patient_domain_means[domain]
        prototype_payload.append({
            "domain": domain,
            "mean": prototypes[domain].tolist(),
            "between_patient_standard_deviation": between.std(axis=0, ddof=1).tolist(),
            "patient_count": len(between),
        })
    fold_records = []
    correct = 0
    brier_sum = 0.0
    for patient in patients:
        training = [sample for sample in samples if sample["patient_id"] != patient]
        heldout = [sample for sample in samples if sample["patient_id"] == patient]
        fold_prototypes, _, fold_inverse = fit_atlas(training, domains, request["covariance_ridge"])
        for sample in heldout:
            squared = distances(np.asarray(sample["features"]), domains, fold_prototypes, fold_inverse)
            probability = probabilities_from_distances(squared)
            prediction = domains[int(np.argmax(probability))]
            correct += prediction == sample["domain"]
            truth = np.asarray([domain == sample["domain"] for domain in domains], dtype=float)
            brier_sum += float(np.mean((probability - truth) ** 2))
            fold_records.append({
                "patient_id": patient,
                "sample_id": sample["sample_id"],
                "observed_domain": sample["domain"],
                "predicted_domain": prediction,
                "domain_probabilities": {domain: float(value) for domain, value in zip(domains, probability)},
            })
    perturbation_results = []
    rng = np.random.default_rng(request["seed"])
    for perturbation in request["perturbations"]:
        shuffled = np.arange(len(samples))
        rng.shuffle(shuffled)
        retained_count = max(1, int(math.ceil(len(samples) * perturbation["subsample_fraction"])))
        selected = set(shuffled[:retained_count].tolist())
        perturbation_correct = 0
        for index, sample in enumerate(samples):
            if index not in selected:
                continue
            training = [other for other in samples if other["patient_id"] != sample["patient_id"]]
            fold_prototypes, _, fold_inverse = fit_atlas(training, domains, request["covariance_ridge"])
            shifted = np.asarray(sample["features"]) + np.asarray(perturbation["feature_shift"])
            squared = distances(shifted, domains, fold_prototypes, fold_inverse)
            perturbation_correct += domains[int(np.argmin(squared))] == sample["domain"]
        perturbation_results.append({
            "perturbation_id": perturbation["perturbation_id"],
            "feature_shift": perturbation["feature_shift"],
            "subsample_fraction": perturbation["subsample_fraction"],
            "evaluated_sample_count": retained_count,
            "accuracy": perturbation_correct / retained_count,
        })
    query_mappings = []
    expected_correct = 0
    expected_count = 0
    threshold = request["alignment"]["ood_distance_threshold"]
    for query in request["queries"]:
        squared = distances(np.asarray(query["features"]), domains, prototypes, inverse)
        conditional = probabilities_from_distances(squared)
        minimum_distance = math.sqrt(float(squared.min()))
        unmatched = 1.0 / (1.0 + math.exp(-2.0 * (minimum_distance - threshold)))
        prediction = domains[int(np.argmax(conditional))]
        if query["expected_domain"] is not None:
            expected_count += 1
            expected_correct += prediction == query["expected_domain"]
        joint = np.concatenate([(1.0 - unmatched) * conditional, [unmatched]])
        entropy = float(-np.sum(joint * np.log(np.maximum(joint, 1e-300))))
        query_mappings.append({
            "query_id": query["query_id"],
            "predicted_domain": prediction,
            "domain_probabilities": {domain: float(value) for domain, value in zip(domains, conditional)},
            "minimum_mahalanobis_distance": minimum_distance,
            "unmatched_probability": unmatched,
            "mapping_entropy": entropy,
            "expected_domain": query["expected_domain"],
        })
    request_sha = hashlib.sha256(request_bytes).hexdigest()
    result = {
        "format": "marklab.spatial_atlas_mapping_and_validation",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": request_sha,
        "atlas": {
            "format": "marklab.spatial_atlas",
            "version": 1,
            "atlas_id": request["atlas_id"],
            "artifact_id": f"sha256:{request_sha}",
            "support_type": "domain_prototypes",
            "physical_registration_claim": False,
            "representation": request["representation"],
            "alignment": request["alignment"],
            "domains": domains,
            "prototypes": prototype_payload,
            "pooled_within_domain_covariance": covariance.tolist(),
            "training_population": {"patient_ids": patients, "site_ids": sites, "sample_count": len(samples)},
            "support_limitations": ["biological_similarity_only", "measured_region_features_only", "synthetic_validation_only"],
        },
        "query_mappings": query_mappings,
        "validation": {
            "split_policy": "leave_one_patient_out_all_preprocessing_inside_fold",
            "leave_one_patient_out_accuracy": correct / len(fold_records),
            "leave_one_patient_out_brier_score": brier_sum / len(fold_records),
            "fold_records": fold_records,
            "query_expected_domain_accuracy": expected_correct / expected_count,
            "perturbations": perturbation_results,
            "calibration_status": "synthetic_only",
        },
        "claim_status": "experimental_synthetic_biological_similarity_atlas",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"atlas worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
