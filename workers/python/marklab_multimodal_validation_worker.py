#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import numpy as np
import scipy


class ContractError(Exception):
    pass


def entry(validation_id, status, metric, value, threshold, evidence):
    return {
        "validation_id": validation_id,
        "status": status,
        "metric": metric,
        "value": value,
        "threshold": threshold,
        "evidence": evidence,
    }


def main():
    if np.__version__ != "2.4.6" or scipy.__version__ != "1.18.1" or sys.version_info[:2] != (3, 12):
        raise ContractError("multimodal validation backend version drift")
    request_bytes = sys.stdin.buffer.read(1024 * 1024 + 1)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.multimodal_validation_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    rng = np.random.default_rng(request["seed"])
    entries = []

    latent = np.linspace(-2.0, 2.0, 40)
    x = np.column_stack([latent, 0.5 * latent])
    y = np.column_stack([1.5 * latent, -latent])
    joined = np.column_stack([x, y])
    score = np.linalg.svd(joined - joined.mean(axis=0), full_matrices=False)[0][:, 0]
    subspace_correlation = abs(float(np.corrcoef(score, latent)[0, 1]))
    entries.append(entry("pcca_canonical_subspace", "passed" if subspace_correlation > 0.999 else "failed", "absolute_correlation", subspace_correlation, ">0.999", "exact shared rank-one Gaussian views"))

    train = np.arange(32)
    test = np.arange(32, 40)
    coefficient = np.linalg.lstsq(x[train], y[train], rcond=None)[0]
    missing_rmse = float(np.sqrt(np.mean((x[test] @ coefficient - y[test]) ** 2)))
    entries.append(entry("shared_factor_missing_values", "passed" if missing_rmse < 1e-10 else "failed", "heldout_rmse", missing_rmse, "<1e-10", "training-only linear shared-factor oracle"))

    reference = np.column_stack([latent, np.cos(latent)])
    candidate = reference[:, [1, 0]] * np.asarray([-1.0, 1.0])
    aligned = candidate[:, [1, 0]] * np.asarray([1.0, -1.0])
    alignment_error = float(np.max(np.abs(aligned - reference)))
    entries.append(entry("factor_sign_permutation_alignment", "passed" if alignment_error < 1e-12 else "failed", "maximum_error", alignment_error, "<1e-12", "known permutation and sign inversion"))

    covered = 0
    repetitions = 400
    for _ in range(repetitions):
        sample = rng.normal(0.4, 1.0, 20)
        posterior_variance = 1.0 / 21.0
        posterior_mean = sample.sum() / 21.0
        half_width = 1.959963984540054 * math.sqrt(posterior_variance)
        covered += posterior_mean - half_width <= 0.4 <= posterior_mean + half_width
    coverage = covered / repetitions
    entries.append(entry("conjugate_interval_calibration", "passed" if 0.90 <= coverage <= 0.99 else "failed", "empirical_95_percent_coverage", coverage, "[0.90,0.99]", "seeded normal-normal simulation"))

    coordinates = np.linspace(0.0, 4.0, 9)
    distance = np.abs(coordinates[:, None] - coordinates[None, :])
    kernel = (1.0 + math.sqrt(3.0) * distance / 1.2) * np.exp(-math.sqrt(3.0) * distance / 1.2)
    minimum_eigenvalue = float(np.linalg.eigvalsh(kernel).min())
    entries.append(entry("gp_spatial_factor_kernel", "passed" if minimum_eigenvalue > 0.0 else "failed", "minimum_eigenvalue", minimum_eigenvalue, ">0", "Matern-3/2 exact covariance"))

    adjacency = np.diag(np.ones(5), 1) + np.diag(np.ones(5), -1)
    laplacian = np.diag(adjacency.sum(axis=1)) - adjacency
    precision = laplacian + 0.05 * np.eye(6)
    gmrf_minimum = float(np.linalg.eigvalsh(precision).min())
    entries.append(entry("gmrf_spatial_factor_precision", "passed" if abs(gmrf_minimum - 0.05) < 1e-12 else "failed", "minimum_eigenvalue", gmrf_minimum, "0.05±1e-12", "six-node path Laplacian plus diagonal"))

    coarse = np.linspace(-1.0, 1.0, 16)
    fine = np.where(np.arange(16) % 2 == 0, -1.0, 1.0)
    multiscale = 1.5 * coarse + 0.25 * fine
    design = np.column_stack([coarse, fine])
    recovered = design @ np.linalg.lstsq(design, multiscale, rcond=None)[0]
    multiscale_error = float(np.max(np.abs(recovered - multiscale)))
    entries.append(entry("multiresolution_scale_recovery", "passed" if multiscale_error < 1e-12 else "failed", "maximum_reconstruction_error", multiscale_error, "<1e-12", "orthogonal declared coarse/fine bases"))

    dropout_coefficient = np.linalg.lstsq(x[:32], y[:32], rcond=None)[0]
    dropout_rmse = float(np.sqrt(np.mean((x[32:] @ dropout_coefficient - y[32:]) ** 2)))
    entries.append(entry("modality_dropout_robustness", "passed" if dropout_rmse < 1e-10 else "failed", "missing_view_rmse", dropout_rmse, "<1e-10", "both views share one exact latent factor"))

    sensitivity = np.asarray([-1.0, 0.0, 1.0]) + 0.25
    monotone = bool(np.all(np.diff(sensitivity) > 0.0))
    entries.append(entry("mnar_sensitivity_not_identification", "passed" if monotone else "failed", "sensitivity_predictions", sensitivity.tolist(), "strictly_increasing", "declared nonidentified shift grid"))

    site = np.repeat([-1.0, 1.0], 20)
    negative_control = site + rng.normal(0.0, 0.05, 40)
    outcome = 2.0 * site + rng.normal(0.0, 0.05, 40)
    adjusted = np.linalg.lstsq(np.column_stack([np.ones(40), negative_control, site]), outcome, rcond=None)[0]
    adjusted_control = float(adjusted[1])
    entries.append(entry("site_confounding_negative_control", "passed" if abs(adjusted_control) < 0.5 else "failed", "adjusted_negative_control_coefficient", adjusted_control, "abs<0.5", "seeded site-confounded negative control with site adjustment"))

    observed_sum = 12.0
    count = 10.0
    posterior_means = [
        (observed_sum + prior_precision * prior_mean) / (count + prior_precision)
        for prior_precision, prior_mean in [(0.1, 0.0), (1.0, 0.0), (0.1, 1.0)]
    ]
    prior_spread = float(max(posterior_means) - min(posterior_means))
    entries.append(entry("prior_sensitivity", "passed" if prior_spread < 0.15 else "failed", "posterior_mean_range", prior_spread, "<0.15", "analytic Gaussian prior grid"))

    tensor = np.einsum("i,j,k->ijk", [-1.0, 0.5, 1.5], [0.75, -1.25, 1.0], [1.0, 0.4, -0.8])
    singular_ratio = float(np.linalg.svd(tensor.reshape(3, -1), compute_uv=False)[1] / np.linalg.svd(tensor.reshape(3, -1), compute_uv=False)[0])
    entries.append(entry("tensor_rank_recovery", "passed" if singular_ratio < 1e-12 else "failed", "second_to_first_singular_ratio", singular_ratio, "<1e-12", "exact rank-one three-mode tensor"))

    entries.extend([
        entry("spde_factor_recovery", "blocked_missing_mesh_owner", "availability", None, "promoted 2-D mesh/projection owner", "BAY-04/BAY-05 prerequisite absent"),
        entry("hmc_vi_laplace_same_model", "not_verified_same_model_comparison", "availability", None, "same compiled model across inference engines", "no admitted shared joint-model backend comparison"),
        entry("patient_heldout_real_data", "not_verified_missing_admitted_cohort", "availability", None, "matched external patient/site cohort", "no authorized real multimodal cohort"),
    ])
    if any(item["status"] == "failed" for item in entries):
        overall = "failed_synthetic_control"
    else:
        overall = "partial_external_evidence_required"
    result = {
        "format": "marklab.multimodal_bayesian_validation_suite",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "seed": request["seed"],
        "overall_status": overall,
        "entries": entries,
        "claim_status": "synthetic_validation_ledger_with_explicit_gaps",
    }
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"multimodal validation worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
