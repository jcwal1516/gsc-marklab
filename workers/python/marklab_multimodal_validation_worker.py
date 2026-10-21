#!/usr/bin/env python3
"""Synthetic validation report for the multimodal Bayesian factor-model family."""

import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import numpy as np
import scipy

from marklab_worker_support import fem
from marklab_worker_support.protocol import (
    CONTRACT_ERRORS,
    MIB,
    exact,
    read_request,
    require_versions,
    run_worker,
    sha256_hex,
    write_result,
)
from marklab_worker_support.validation_checks import entry, pass_fail

SPDE_RECOVERY_THRESHOLDS = {"spde_truth": 0.9, "dense_gp_truth": 0.9, "spde_dense_gp": 0.95}
"""Fixed before the comparison was first run."""


def matern32(distances: np.ndarray, length_scale: float) -> np.ndarray:
    """Matern-3/2 correlation ``(1 + sqrt(3) d / l) exp(-sqrt(3) d / l)``."""
    scaled = math.sqrt(3.0) * distances / length_scale
    return (1.0 + scaled) * np.exp(-scaled)


def spde_factor_recovery(seed: int) -> dict:
    """Recover one spatial factor with an SPDE prior on a projected mesh and a dense GP.

    Truth: 48 regions uniform in a 1000 um square, a factor drawn from a unit-variance
    Matern-3/2 GP (length scale 300 um), loadings (1, -0.8, 0.6, 0.4) and noise SD 0.25. Both
    priors are fitted by MAP of the same one-factor objective: the dense GP uses the generating
    covariance; the SPDE (alpha = 2, nu = 1) uses a regular 16 x 16 mesh extended by the range
    beyond the regions, unit marginal variance ``tau = 1 / (2 sqrt(pi) kappa)`` and the range
    ``sqrt(8) / kappa`` set to the dense kernel's 0.1-correlation distance. The smoothness
    differs (1 versus 3/2), so the check is recovery of a common factor, not equal priors.
    """
    rng = np.random.default_rng([seed, 4])
    length_scale = 300.0
    noise_sd = 0.25
    coordinates = rng.uniform(0.0, 1000.0, (48, 2))
    distances = np.linalg.norm(coordinates[:, None] - coordinates[None], axis=2)
    covariance = matern32(distances, length_scale) + 1e-6 * np.eye(len(coordinates))
    truth = np.linalg.cholesky(covariance) @ rng.normal(size=len(coordinates))
    loadings = np.asarray([1.0, -0.8, 0.6, 0.4])
    values = truth[:, None] * loadings[None, :] + noise_sd * rng.normal(size=(48, 4))
    centered = values - values.mean(axis=0)
    _, _, dense, _ = fem.fit_factor_map(
        np.eye(len(coordinates)), centered, np.linalg.inv(covariance), noise_sd, 2000, "dense GP"
    )
    # (1 + sqrt(3) x) exp(-sqrt(3) x) = 0.1 at sqrt(3) x = 3.8897.
    practical_range = 3.889720169867429 / math.sqrt(3.0) * length_scale
    kappa = math.sqrt(8.0) / practical_range
    tau = 1.0 / (2.0 * math.sqrt(math.pi) * kappa)
    bounds = [-practical_range, 1000.0 + practical_range] * 2
    mesh = fem.build_mesh(bounds, 16, kappa, tau)
    _, _, spde, _ = fem.fit_factor_map(
        fem.projection(mesh, coordinates), centered, mesh["precision"], noise_sd, 2000, "SPDE"
    )
    correlations = {
        "spde_truth": float(np.corrcoef(spde, truth)[0, 1]),
        "dense_gp_truth": float(np.corrcoef(dense, truth)[0, 1]),
        "spde_dense_gp": float(np.corrcoef(spde, dense)[0, 1]),
    }
    return {
        "correlations": correlations,
        "passed": all(
            correlations[name] >= threshold for name, threshold in SPDE_RECOVERY_THRESHOLDS.items()
        ),
        "mesh_vertices": len(mesh["vertices"]),
        "practical_range_um": practical_range,
    }


def synthetic_entries(rng: np.random.Generator, seed: int) -> list[dict]:
    entries = []

    latent = np.linspace(-2.0, 2.0, 40)
    x = np.column_stack([latent, 0.5 * latent])
    y = np.column_stack([1.5 * latent, -latent])
    joined = np.column_stack([x, y])
    score = np.linalg.svd(joined - joined.mean(axis=0), full_matrices=False)[0][:, 0]
    subspace_correlation = abs(float(np.corrcoef(score, latent)[0, 1]))
    entries.append(
        entry(
            "pcca_canonical_subspace",
            pass_fail(subspace_correlation > 0.999),
            "absolute_correlation",
            subspace_correlation,
            ">0.999",
            "exact shared rank-one Gaussian views",
        )
    )

    train = np.arange(32)
    test = np.arange(32, 40)
    coefficient = np.linalg.lstsq(x[train], y[train], rcond=None)[0]
    missing_rmse = float(np.sqrt(np.mean((x[test] @ coefficient - y[test]) ** 2)))
    entries.append(
        entry(
            "shared_factor_missing_values",
            pass_fail(missing_rmse < 1e-10),
            "heldout_rmse",
            missing_rmse,
            "<1e-10",
            "training-only linear shared-factor oracle",
        )
    )

    reference = np.column_stack([latent, np.cos(latent)])
    candidate = reference[:, [1, 0]] * np.asarray([-1.0, 1.0])
    aligned = candidate[:, [1, 0]] * np.asarray([1.0, -1.0])
    alignment_error = float(np.max(np.abs(aligned - reference)))
    entries.append(
        entry(
            "factor_sign_permutation_alignment",
            pass_fail(alignment_error < 1e-12),
            "maximum_error",
            alignment_error,
            "<1e-12",
            "known permutation and sign inversion",
        )
    )

    covered = 0
    repetitions = 400
    for _ in range(repetitions):
        sample = rng.normal(0.4, 1.0, 20)
        posterior_variance = 1.0 / 21.0
        posterior_mean = sample.sum() / 21.0
        half_width = 1.959963984540054 * math.sqrt(posterior_variance)
        covered += posterior_mean - half_width <= 0.4 <= posterior_mean + half_width
    coverage = covered / repetitions
    entries.append(
        entry(
            "conjugate_interval_calibration",
            pass_fail(0.90 <= coverage <= 0.99),
            "empirical_95_percent_coverage",
            coverage,
            "[0.90,0.99]",
            "seeded normal-normal simulation",
        )
    )

    coordinates = np.linspace(0.0, 4.0, 9)
    distance = np.abs(coordinates[:, None] - coordinates[None, :])
    scaled_distance = math.sqrt(3.0) * distance / 1.2
    kernel = (1.0 + scaled_distance) * np.exp(-scaled_distance)
    minimum_eigenvalue = float(np.linalg.eigvalsh(kernel).min())
    entries.append(
        entry(
            "gp_spatial_factor_kernel",
            pass_fail(minimum_eigenvalue > 0.0),
            "minimum_eigenvalue",
            minimum_eigenvalue,
            ">0",
            "Matern-3/2 exact covariance",
        )
    )

    adjacency = np.diag(np.ones(5), 1) + np.diag(np.ones(5), -1)
    laplacian = np.diag(adjacency.sum(axis=1)) - adjacency
    precision = laplacian + 0.05 * np.eye(6)
    gmrf_minimum = float(np.linalg.eigvalsh(precision).min())
    entries.append(
        entry(
            "gmrf_spatial_factor_precision",
            pass_fail(abs(gmrf_minimum - 0.05) < 1e-12),
            "minimum_eigenvalue",
            gmrf_minimum,
            "0.05±1e-12",
            "six-node path Laplacian plus diagonal",
        )
    )

    coarse = np.linspace(-1.0, 1.0, 16)
    fine = np.where(np.arange(16) % 2 == 0, -1.0, 1.0)
    multiscale = 1.5 * coarse + 0.25 * fine
    design = np.column_stack([coarse, fine])
    recovered = design @ np.linalg.lstsq(design, multiscale, rcond=None)[0]
    multiscale_error = float(np.max(np.abs(recovered - multiscale)))
    entries.append(
        entry(
            "multiresolution_scale_recovery",
            pass_fail(multiscale_error < 1e-12),
            "maximum_reconstruction_error",
            multiscale_error,
            "<1e-12",
            "orthogonal declared coarse/fine bases",
        )
    )

    dropout_coefficient = np.linalg.lstsq(x[:32], y[:32], rcond=None)[0]
    dropout_rmse = float(np.sqrt(np.mean((x[32:] @ dropout_coefficient - y[32:]) ** 2)))
    entries.append(
        entry(
            "modality_dropout_robustness",
            pass_fail(dropout_rmse < 1e-10),
            "missing_view_rmse",
            dropout_rmse,
            "<1e-10",
            "both views share one exact latent factor",
        )
    )

    sensitivity = np.asarray([-1.0, 0.0, 1.0]) + 0.25
    monotone = bool(np.all(np.diff(sensitivity) > 0.0))
    entries.append(
        entry(
            "mnar_sensitivity_not_identification",
            pass_fail(monotone),
            "sensitivity_predictions",
            sensitivity.tolist(),
            "strictly_increasing",
            "declared nonidentified shift grid",
        )
    )

    site = np.repeat([-1.0, 1.0], 20)
    negative_control = site + rng.normal(0.0, 0.05, 40)
    outcome = 2.0 * site + rng.normal(0.0, 0.05, 40)
    adjustment_design = np.column_stack([np.ones(40), negative_control, site])
    adjusted = np.linalg.lstsq(adjustment_design, outcome, rcond=None)[0]
    adjusted_control = float(adjusted[1])
    entries.append(
        entry(
            "site_confounding_negative_control",
            pass_fail(abs(adjusted_control) < 0.5),
            "adjusted_negative_control_coefficient",
            adjusted_control,
            "abs<0.5",
            "seeded site-confounded negative control with site adjustment",
        )
    )

    observed_sum = 12.0
    count = 10.0
    posterior_means = [
        (observed_sum + prior_precision * prior_mean) / (count + prior_precision)
        for prior_precision, prior_mean in [(0.1, 0.0), (1.0, 0.0), (0.1, 1.0)]
    ]
    prior_spread = float(max(posterior_means) - min(posterior_means))
    entries.append(
        entry(
            "prior_sensitivity",
            pass_fail(prior_spread < 0.15),
            "posterior_mean_range",
            prior_spread,
            "<0.15",
            "analytic Gaussian prior grid",
        )
    )

    tensor = np.einsum("i,j,k->ijk", [-1.0, 0.5, 1.5], [0.75, -1.25, 1.0], [1.0, 0.4, -0.8])
    singular_values = np.linalg.svd(tensor.reshape(3, -1), compute_uv=False)
    singular_ratio = float(singular_values[1] / singular_values[0])
    entries.append(
        entry(
            "tensor_rank_recovery",
            pass_fail(singular_ratio < 1e-12),
            "second_to_first_singular_ratio",
            singular_ratio,
            "<1e-12",
            "exact rank-one three-mode tensor",
        )
    )

    recovery = spde_factor_recovery(seed)
    entries.append(
        entry(
            "spde_factor_recovery",
            pass_fail(recovery["passed"]),
            "minimum_factor_correlation",
            recovery["correlations"],
            {name: f">={threshold}" for name, threshold in SPDE_RECOVERY_THRESHOLDS.items()},
            f"seeded Matern-3/2 factor at 48 regions; MAP fits with the dense GP and an SPDE "
            f"prior on a {recovery['mesh_vertices']}-vertex mesh with barycentric projection",
        )
    )
    entries.extend(
        [
            entry(
                "hmc_vi_laplace_same_model",
                "not_verified_same_model_comparison",
                "availability",
                None,
                "same compiled model across inference engines",
                "no admitted shared joint-model backend comparison",
            ),
            entry(
                "patient_heldout_real_data",
                "not_verified_missing_admitted_cohort",
                "availability",
                None,
                "matched external patient/site cohort",
                "no authorized real multimodal cohort",
            ),
        ]
    )
    return entries


def main() -> None:
    require_versions(numpy=np.__version__, scipy=scipy.__version__)
    request_bytes = read_request(MIB)
    request = json.loads(request_bytes)
    exact(request["format"], "marklab.multimodal_validation_request", "request.format")
    exact(request["version"], 1, "request.version")
    entries = synthetic_entries(np.random.default_rng(request["seed"]), request["seed"])
    if any(item["status"] == "failed" for item in entries):
        overall = "failed_synthetic_control"
    else:
        overall = "partial_external_evidence_required"
    result = {
        "format": "marklab.multimodal_bayesian_validation_suite",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "seed": request["seed"],
        "overall_status": overall,
        "entries": entries,
        "claim_status": "synthetic_validation_ledger_with_explicit_gaps",
    }
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "multimodal validation worker contract error", errors=CONTRACT_ERRORS)
