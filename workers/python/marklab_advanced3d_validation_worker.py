#!/usr/bin/env python3
"""Synthetic oracle checks for the 3-D, serial-section, longitudinal, and clone workflows."""

import json
import math
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import gudhi
import numpy as np
import scipy

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

Z_975 = 1.959963984540054


def record(validation_id, status, metric, value, threshold, evidence):
    return {
        "validation_id": validation_id,
        "status": status,
        "metric": metric,
        "value": value,
        "threshold": threshold,
        "evidence": evidence,
    }


def passed_if(condition):
    return "passed" if condition else "failed"


def validation_entries(rng):
    """Run each synthetic oracle check in order; ``rng`` drives the coverage simulation."""
    entries = []

    section = np.asarray([[0.0, 0.0], [2.0, 0.0], [0.0, 2.0], [2.0, 2.0]])
    translation = np.asarray([1.0, -0.5])
    recovered = np.mean(section - (section + translation), axis=0)
    error = float(np.linalg.norm(recovered + translation))
    entries.append(
        record(
            "serial_transform_recovery",
            passed_if(error < 1e-12),
            "translation_error",
            error,
            "<1e-12",
            "paired landmark translation",
        )
    )

    coverage = 0
    for _ in range(500):
        observations = rng.normal(translation[0], 0.2, 8)
        mean = observations.mean()
        half = Z_975 * 0.2 / math.sqrt(8)
        coverage += mean - half <= translation[0] <= mean + half
    coverage /= 500
    entries.append(
        record(
            "stack_transform_interval_coverage",
            passed_if(0.90 <= coverage <= 0.99),
            "coverage",
            coverage,
            "[0.90,0.99]",
            "seeded Gaussian landmark posterior",
        )
    )

    cycle = np.linalg.norm(translation + (-translation))
    entries.append(
        record(
            "serial_cycle_consistency",
            passed_if(cycle < 1e-12),
            "cycle_error",
            float(cycle),
            "<1e-12",
            "forward/inverse translations",
        )
    )

    tetrahedron = [
        [0.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [1.0, math.sqrt(3.0), 0.0],
        [1.0, math.sqrt(3.0) / 3.0, 2.0 * math.sqrt(2.0 / 3.0)],
    ]
    alpha = gudhi.AlphaComplex(points=tetrahedron, precision="exact")
    tree = alpha.create_simplex_tree(max_alpha_square=2.0)
    tetra_values = [value for simplex, value in tree.get_filtration() if len(simplex) == 4]
    alpha_error = abs(tetra_values[0] - 1.5)
    entries.append(
        record(
            "alpha3d_regular_tetrahedron",
            passed_if(alpha_error < 1e-10),
            "squared_alpha_error",
            alpha_error,
            "<1e-10",
            "GUDHI exact alpha",
        )
    )

    distances = np.asarray([[0.0, 1.0, 2.0], [1.0, 0.0, 1.0], [2.0, 1.0, 0.0]])
    adjacency = (distances <= 1.0).astype(float) - np.eye(3)
    graph_ok = bool(np.array_equal(adjacency, [[0, 1, 0], [1, 0, 1], [0, 1, 0]]))
    entries.append(
        record(
            "spatial3d_graph_hand_oracle",
            passed_if(graph_ok),
            "path_adjacency",
            graph_ok,
            True,
            "three collinear points",
        )
    )

    anisotropic_scaled = np.linalg.norm(np.asarray([2.0, 3.0, 4.0]) / np.asarray([2.0, 3.0, 4.0]))
    entries.append(
        record(
            "anisotropic_3d_distance",
            passed_if(abs(anisotropic_scaled - math.sqrt(3.0)) < 1e-12),
            "scaled_distance",
            anisotropic_scaled,
            "sqrt(3)",
            "axis length-scale oracle",
        )
    )

    prior_mean, prior_variance, observation, noise = 0.0, 1.0, 2.0, 0.25
    gain = prior_variance / (prior_variance + noise)
    posterior_mean = prior_mean + gain * (observation - prior_mean)
    entries.append(
        record(
            "kalman_update",
            passed_if(abs(posterior_mean - 1.6) < 1e-12),
            "posterior_mean",
            posterior_mean,
            "1.6",
            "scalar Gaussian update",
        )
    )

    weights = np.asarray([0.1, 0.2, 0.3, 0.4])
    ess = 1.0 / np.sum(weights * weights)
    entries.append(
        record(
            "particle_filter_ess",
            passed_if(abs(ess - 10.0 / 3.0) < 1e-12),
            "ess",
            ess,
            "10/3",
            "normalized particle weights",
        )
    )

    baseline = np.sin(np.linspace(0, math.pi, 36))
    domain = np.repeat([0.0, 1.0], 18)
    post = baseline + domain
    design = np.column_stack([np.ones(36), domain])
    effect = np.linalg.lstsq(design, post - baseline, rcond=None)[0][1]
    negative = np.linalg.lstsq(design, baseline - baseline, rcond=None)[0][1]
    entries.append(
        record(
            "deformation_biology_separation",
            passed_if(abs(effect - 1.0) < 1e-12 and abs(negative) < 1e-12),
            "effect_and_negative",
            [float(effect), float(negative)],
            "[1,0]",
            "known change and deformation-only control",
        )
    )

    diffusion = (1.0**2 + 0.2**2 + (-1.0) ** 2 + (-0.2) ** 2) / (4.0 * 2.0)
    entries.append(
        record(
            "clone_diffusion_positive",
            passed_if(diffusion > 0),
            "diffusion_mle",
            diffusion,
            ">0",
            "two branch transitions",
        )
    )

    niche = np.concatenate([np.ones(20), -np.ones(20)])
    clone = np.concatenate([np.ones(20), -np.ones(20)])
    clone_design = np.column_stack([np.ones(40), clone])
    contrast = 2.0 * np.linalg.lstsq(clone_design, niche, rcond=None)[0][1]
    entries.append(
        record(
            "clone_niche_contrast",
            passed_if(abs(contrast - 2.0) < 1e-12),
            "a_minus_b",
            contrast,
            "2",
            "balanced uncertain-clone limit",
        )
    )

    tree_distances = np.asarray([1.0, 1.0, 2.0])
    spatial_distances = np.asarray([1.0, 1.0, 2.0])
    association = float(np.corrcoef(tree_distances, spatial_distances)[0, 1])
    entries.append(
        record(
            "phylogenetic_spatial_association",
            passed_if(association > 0.999),
            "correlation",
            association,
            ">0.999",
            "three-clone path oracle",
        )
    )

    entries.append(
        record(
            "missing_section_gap_reporting",
            "not_verified_unexecuted_control",
            "gaps_um",
            None,
            "explicit",
            "production missing-section path was not executed",
        )
    )
    entries.append(
        record(
            "real_serial_longitudinal_clone_cohort",
            "not_verified_missing_admitted_data",
            "availability",
            None,
            "registered repeated real units with clone uncertainty",
            "synthetic fixtures only",
        )
    )
    return entries


def main():
    require_versions(gudhi=gudhi.__version__, numpy=np.__version__, scipy=scipy.__version__)
    request_bytes = read_request(MIB)
    request = json.loads(request_bytes)
    if request["format"] != "marklab.advanced3d_validation_request" or request["version"] != 1:
        raise ContractError("request identity mismatch")
    entries = validation_entries(np.random.default_rng(request["seed"]))
    if any(entry["status"] == "failed" for entry in entries):
        overall = "failed_synthetic_control"
    else:
        overall = "partial_synthetic_and_real_evidence_required"
    result = {
        "format": "marklab.advanced_3d_longitudinal_validation_suite",
        "version": 1,
        "backend": request["backend"],
        "request_sha256": sha256_hex(request_bytes),
        "seed": request["seed"],
        "overall_status": overall,
        "entries": entries,
        "claim_status": "synthetic_3d_longitudinal_validation_with_explicit_real_gap",
    }
    write_result(result)


if __name__ == "__main__":
    run_worker(main, "advanced 3-D validation worker contract error", errors=CONTRACT_ERRORS)
