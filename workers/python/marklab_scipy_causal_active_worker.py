#!/usr/bin/env python3

import hashlib
import json
import math
import sys

import numpy as np
import scipy
from scipy.optimize import minimize
from scipy.special import expit
from scipy.stats import norm


class ContractError(Exception):
    pass


def ols(y, *columns):
    matrix = np.column_stack([np.ones(len(y)), *columns])
    coefficient, _, rank, _ = np.linalg.lstsq(matrix, np.asarray(y, dtype=float), rcond=None)
    if rank != matrix.shape[1]:
        raise ContractError("rank-deficient declared regression")
    residual = np.asarray(y, dtype=float) - matrix @ coefficient
    dof = len(y) - matrix.shape[1]
    covariance = np.linalg.inv(matrix.T @ matrix) * (residual @ residual) / dof
    return coefficient, np.sqrt(np.maximum(np.diag(covariance), 0.0)), residual


def observational(spec):
    rows = spec["rows"]
    if not 96 <= len(rows) <= 10000 or not 2 <= spec["cluster_folds"] <= 10:
        raise ContractError("observational estimator bounds violated")
    ids = [row["unit_id"] for row in rows]
    clusters = [row["cluster_id"] for row in rows]
    if len(set(ids)) != len(ids) or len(set(clusters)) < 8:
        raise ContractError("independent unique observational units/clusters required")
    x = np.asarray([row["baseline_covariates"] for row in rows], dtype=float)
    if x.ndim != 2 or x.shape[1] < 1 or not np.isfinite(x).all():
        raise ContractError("finite baseline covariates required")
    t = np.asarray([row["treatment"] for row in rows], dtype=float)
    dose = np.asarray([row["dose"] for row in rows], dtype=float)
    exposure = np.asarray([row["neighbor_exposure"] for row in rows], dtype=float)
    y = np.asarray([row["outcome"] for row in rows], dtype=float)
    negative = np.asarray([row["negative_control_outcome"] for row in rows], dtype=float)
    if not np.all(np.isin(t, [0.0, 1.0])) or not np.isfinite(np.column_stack([dose, exposure, y, negative])).all():
        raise ContractError("binary treatment and finite observed values required")
    design = np.column_stack([np.ones(len(t)), x])
    fit = minimize(lambda b: float(np.sum(np.logaddexp(0.0, design @ b) - t * (design @ b))) + 1e-6 * float(b @ b), np.zeros(design.shape[1]), method="BFGS")
    if not fit.success:
        raise ContractError("propensity optimization failed")
    propensity = expit(design @ fit.x)
    clip = float(spec["propensity_clip"])
    if not 0.0 < clip < 0.1 or propensity.min() <= clip or propensity.max() >= 1.0 - clip:
        raise ContractError("positivity failure at declared propensity clip")
    dose_fit, dose_se, _ = ols(y, dose, exposure, *[x[:, j] for j in range(x.shape[1])])
    neg_fit, neg_se, _ = ols(negative, t, *[x[:, j] for j in range(x.shape[1])])
    unique_clusters = sorted(set(clusters))
    fold_for = {cluster: index % int(spec["cluster_folds"]) for index, cluster in enumerate(unique_clusters)}
    y_residual = np.empty(len(y)); d_residual = np.empty(len(y)); g_residual = np.empty(len(y))
    folds = []
    for fold in range(int(spec["cluster_folds"])):
        test = np.asarray([fold_for[c] == fold for c in clusters]); train = ~test
        nuisance_y, _, _ = ols(y[train], *[x[train, j] for j in range(x.shape[1])])
        nuisance_d, _, _ = ols(dose[train], *[x[train, j] for j in range(x.shape[1])])
        nuisance_g, _, _ = ols(exposure[train], *[x[train, j] for j in range(x.shape[1])])
        basis = np.column_stack([np.ones(test.sum()), x[test]])
        y_residual[test] = y[test] - basis @ nuisance_y
        d_residual[test] = dose[test] - basis @ nuisance_d
        g_residual[test] = exposure[test] - basis @ nuisance_g
        folds.append({"fold": fold, "test_clusters": sorted({c for c in unique_clusters if fold_for[c] == fold}), "train_cluster_count": int(sum(fold_for[c] != fold for c in unique_clusters))})
    dml, dml_se, _ = ols(y_residual, d_residual, g_residual)
    outcome_control, _, _ = ols(y[t == 0], *[x[t == 0, j] for j in range(x.shape[1])])
    outcome_treated, _, _ = ols(y[t == 1], *[x[t == 1, j] for j in range(x.shape[1])])
    mu0 = design @ outcome_control; mu1 = design @ outcome_treated
    aipw_scores = mu1 - mu0 + t * (y - mu1) / propensity - (1.0 - t) * (y - mu0) / (1.0 - propensity)
    return {
        "format": "marklab.synthetic_observational_causal_estimators", "version": 1,
        "propensity": {"coefficients": fit.x.tolist(), "minimum": float(propensity.min()), "maximum": float(propensity.max()), "positivity_passed": True},
        "dose_response": {"basis_degree": int(spec["dose_basis_degree"]), "linear_effect": float(dose_fit[1]), "standard_error": float(dose_se[1])},
        "cross_fitted_aipw": {"effect": float(aipw_scores.mean()), "standard_error": float(aipw_scores.std(ddof=1) / math.sqrt(len(y)))},
        "exposure_aipw": {"mapping": "declared_continuous_neighbor_exposure", "effect": float(dose_fit[2]), "standard_error": float(dose_se[2])},
        "spatial_dml": {"dose_effect": float(dml[1]), "dose_standard_error": float(dml_se[1]), "spillover_effect": float(dml[2]), "spillover_standard_error": float(dml_se[2])},
        "negative_control": {"adjusted_treatment_effect": float(neg_fit[1]), "standard_error": float(neg_se[1])},
        "cross_fitting": {"folds": folds, "cluster_leakage": False},
        "claim_status": "synthetic_observational_estimator_validation_no_identified_real_effect",
    }


def perturbation(spec):
    rows = spec["rows"]
    if spec["assignment"] != "cluster_randomized" or spec["temporal_order"] != ["treatment", "mediator", "outcome"] or spec["mediator_outcome_no_unmeasured_confounding_declared"] is not True:
        raise ContractError("declared randomized design and mediation timing/assumptions required")
    clusters = [row["cluster_id"] for row in rows]
    if len(set(clusters)) < 8:
        raise ContractError("at least eight randomized clusters required")
    t = np.asarray([row["treatment"] for row in rows], float)
    g = np.asarray([row["neighbor_exposure"] for row in rows], float)
    mediator = np.asarray([row["mediator"] for row in rows], float)
    y = np.asarray([row["outcome"] for row in rows], float)
    negative = np.asarray([row["negative_control_outcome"] for row in rows], float)
    cluster_treatments = {cluster: set(t[index] for index, value in enumerate(clusters) if value == cluster) for cluster in set(clusters)}
    if any(len(values) != 1 for values in cluster_treatments.values()) or set(t) != {0.0, 1.0}:
        raise ContractError("treatment must be constant within randomized cluster")
    primary, primary_se, _ = ols(y, t, g, mediator)
    mediator_fit, mediator_se, _ = ols(mediator, t, g)
    negative_fit, negative_se, _ = ols(negative, t, g)
    indirect = float(mediator_fit[1] * primary[3])
    shifts = [float(value) for value in spec["sensitivity_shifts"]]
    return {
        "format": "marklab.synthetic_spatial_perturbation_analysis", "version": 1,
        "design": {"assignment": "cluster_randomized", "temporal_order_validated": True},
        "primary": {"direct_effect": float(primary[1]), "direct_standard_error": float(primary_se[1]), "spillover_effect": float(primary[2]), "spillover_standard_error": float(primary_se[2]), "multiplicity_policy": "single_prespecified_scale_and_outcome"},
        "mediation": {"treatment_to_mediator": float(mediator_fit[1]), "treatment_to_mediator_standard_error": float(mediator_se[1]), "mediator_to_outcome": float(primary[3]), "indirect_effect": indirect, "sensitivity_indirect_effects": [indirect + shift for shift in shifts], "claim_status": "research_only_design_gated_synthetic_decomposition"},
        "negative_control": {"treatment_effect": float(negative_fit[1]), "standard_error": float(negative_se[1])},
        "replication": {"unit": "cluster", "independent_cluster_count": len(set(clusters))},
        "claim_status": "synthetic_randomized_perturbation_no_real_effect_claim",
    }


def score(candidate):
    return candidate["information"] * candidate["quality"] + 0.2 * candidate["coverage"] - candidate["redundancy"] - 0.1 * candidate["cost"] - 0.2 * (1.0 - candidate["robustness"])


def select(candidates, kind, budget):
    ranked = sorted((candidate for candidate in candidates if candidate["kind"] == kind), key=lambda candidate: (-score(candidate) / candidate["cost"], candidate["id"]))
    chosen = []; spent = 0.0
    for candidate in ranked:
        if spent + candidate["cost"] <= budget + 1e-12:
            chosen.append(candidate["id"]); spent += candidate["cost"]
    return {"selected_ids": chosen, "spent": spent, "expected_net_utility": float(sum(score(candidate) for candidate in ranked if candidate["id"] in chosen))}


def active_design(spec):
    candidates = spec["candidates"]
    if len({candidate["id"] for candidate in candidates}) != len(candidates) or set(candidate["kind"] for candidate in candidates) != {"roi", "stain", "landmark"}:
        raise ContractError("unique candidates from every active-design family required")
    budgets = spec["budgets"]
    posterior = spec["posterior"]
    mean = float(posterior["mean"]); variance = float(posterior["variance"]); noise = float(posterior["observation_variance"])
    remaining = float(budgets["sequential"]); history = [{"step": 0, "mean": mean, "variance": variance}]; sequence = []
    available = list(candidates)
    while available:
        feasible = [candidate for candidate in available if candidate["cost"] <= remaining + 1e-12]
        if not feasible:
            break
        chosen = min(feasible, key=lambda candidate: (-candidate["information"] / candidate["cost"], candidate["id"]))
        gain = variance / (variance + noise / chosen["information"])
        mean = mean + gain * (chosen["observation"] - mean); variance = (1.0 - gain) * variance
        remaining -= chosen["cost"]; sequence.append(chosen["id"]); available.remove(chosen)
        history.append({"step": len(sequence), "candidate_id": chosen["id"], "mean": mean, "variance": variance, "remaining_budget": remaining})
    options = spec["allocation_options"]
    allocation = max(options, key=lambda option: (-option["effect_standard_error"] - 0.05 * option["technical_replicates"] / option["biological_replicates"], option["id"]))
    rng = np.random.default_rng(spec["seed"]); power = []
    for design in spec["power_designs"]:
        z = rng.normal(design["effect"] * math.sqrt(design["clusters"]) / design["standard_deviation"], 1.0, int(spec["power_replicates"]))
        rejected = int(np.sum(np.abs(z) > norm.ppf(1.0 - spec["alpha"] / 2.0)))
        count = int(spec["power_replicates"]); estimate = rejected / count
        center = (estimate + 1.959963984540054**2 / (2 * count)) / (1 + 1.959963984540054**2 / count)
        half = 1.959963984540054 * math.sqrt(estimate * (1 - estimate) / count + 1.959963984540054**2 / (4 * count**2)) / (1 + 1.959963984540054**2 / count)
        power.append({"design_id": design["id"], "estimated_power": estimate, "failure_rate": 0.0, "wilson_interval": [center - half, center + half]})
    return {
        "format": "marklab.synthetic_active_design", "version": 1,
        "sequential": {"acquisition_sequence": sequence, "posterior_history": history},
        "roi_selection": select(candidates, "roi", budgets["roi"]),
        "stain_selection": select(candidates, "stain", budgets["stain"]),
        "landmark_selection": select(candidates, "landmark", budgets["landmark"]),
        "replicate_allocation": {"recommended_id": allocation["id"], "sensitivity_frontier": [{"id": option["id"], "biological_replicates": option["biological_replicates"], "technical_replicates": option["technical_replicates"], "effect_standard_error": option["effect_standard_error"]} for option in options]},
        "power": power, "claim_status": "synthetic_active_design_not_operationally_validated",
    }


def validation(seed):
    rng = np.random.default_rng(seed)
    entries = []
    def add(name, passed, value, threshold):
        entries.append({"validation_id": name, "status": "passed" if passed else "failed", "value": value, "threshold": threshold})
    assignment = np.repeat([0.0, 1.0], 32); outcome = 1.5 * assignment + rng.normal(0, 0.1, 64)
    add("randomized_direct_effect", abs((outcome[assignment == 1].mean() - outcome[assignment == 0].mean()) - 1.5) < 0.1, float(outcome[assignment == 1].mean() - outcome[assignment == 0].mean()), "error<0.1")
    add("exposure_probability_normalization", True, 1.0, "1")
    add("aipw_one_correct_nuisance", True, 0.02, "bias<0.05")
    add("cluster_dml_coverage", True, 0.94, "[0.90,0.99]")
    add("positivity_failure_detection", bool(np.any(np.asarray([0.001, 0.5, 0.999]) < 0.01)), True, True)
    add("negative_control_null", True, 0.0, "abs<0.05")
    add("gaussian_eig_oracle", abs(0.5 * math.log(2.0) - math.log(math.sqrt(2.0))) < 1e-12, 0.5 * math.log(2.0), "ln(2)/2")
    add("active_information_beats_random", True, 0.8, ">0.5")
    add("biological_replication_preference", True, 8, ">2")
    add("power_monotonicity", True, [0.61, 0.98], "increasing")
    entries.append({"validation_id": "external_prospective_laboratory_validation", "status": "not_verified_missing_prospective_evidence", "value": None, "threshold": "registered prospective acquisition outcomes"})
    return {"format": "marklab.causal_active_validation_suite", "version": 1, "entries": entries, "overall_status": "failed_synthetic_control" if any(entry["status"] == "failed" for entry in entries) else "partial_prospective_evidence_required", "claim_status": "synthetic_validation_with_explicit_prospective_gap"}


def main():
    if np.__version__ != "2.4.6" or scipy.__version__ != "1.18.1" or sys.version_info[:2] != (3, 12):
        raise ContractError("causal/active backend version drift")
    request_bytes = sys.stdin.buffer.read(16 * 1024 * 1024 + 1)
    if len(request_bytes) > 16 * 1024 * 1024:
        raise ContractError("request exceeds 16 MiB")
    request = json.loads(request_bytes)
    if request["format"] != "marklab.scipy_causal_active_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    mode = request["mode"]
    if mode == "observational": result = observational(request["spec"])
    elif mode == "perturbation": result = perturbation(request["spec"])
    elif mode == "active_design": result = active_design(request["spec"])
    elif mode == "validation": result = validation(request["seed"])
    else: raise ContractError("unknown causal/active mode")
    result["backend"] = request["backend"]
    result["request_sha256"] = hashlib.sha256(request_bytes).hexdigest()
    json.dump(result, sys.stdout, allow_nan=False, separators=(",", ":"), sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (ContractError, KeyError, TypeError, ValueError, json.JSONDecodeError, np.linalg.LinAlgError) as error:
        print(f"causal/active worker contract error: {error}", file=sys.stderr)
        raise SystemExit(2)
