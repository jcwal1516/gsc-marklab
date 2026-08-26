use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    cellular_complex_workflow, graph_chebyshev_heat_workflow, graph_diffusion_wavelet_workflow,
    graph_scattering_workflow, graph_spectral_workflow, graph_wavelet_workflow,
    hypergraph_signal_workflow, simplicial_hodge_workflow, typed_triangle_motif_workflow,
    GraphError,
};

#[derive(Clone, Debug, Serialize)]
pub struct GraphValidationEntry {
    pub id: &'static str,
    pub category: &'static str,
    pub algorithm: &'static str,
    pub status: &'static str,
    pub evidence: Value,
    pub limitation: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphMathematicsValidationResult {
    pub format: &'static str,
    pub version: u32,
    pub suite_digest: String,
    pub exact_fixture_status: &'static str,
    pub scientific_stress_status: &'static str,
    pub entries: Vec<GraphValidationEntry>,
    pub claim_status: &'static str,
}

pub fn validate_graph_mathematics_suite() -> Result<GraphMathematicsValidationResult, GraphError> {
    let mut entries = Vec::new();

    let spectral = graph_spectral_workflow(fixture(base_graph(1.1))?)?;
    require(
        spectral.spectrum.eigenvalues.len() == 3
            && spectral
                .spectrum
                .eigenvalues
                .iter()
                .zip([0.0, 1.0, 3.0])
                .all(|(actual, expected)| (actual - expected).abs() <= 1e-10)
            && spectral.spectrum.reconstruction_max_abs_error <= 1e-10,
        "dense path Laplacian/eigenpair oracle",
    )?;
    entries.push(passed(
        "dense_path_laplacian_eigenpairs",
        "exact_fixture",
        "canonical_graph_fourier",
        json!({
            "eigenvalues": spectral.spectrum.eigenvalues,
            "reconstruction_max_abs_error": spectral.spectrum.reconstruction_max_abs_error,
        }),
    ));

    let chebyshev = graph_chebyshev_heat_workflow(fixture(json!({
        "graph": base_graph(1.1),
        "time": 1.0,
        "tolerance": 1e-8,
        "maximum_order": 32,
    }))?)?;
    require(
        chebyshev.maximum_signal_error <= 1e-8 && chebyshev.verified_grid_error <= 1e-8,
        "Chebyshev/exact heat differential oracle",
    )?;
    entries.push(passed(
        "chebyshev_against_exact_heat",
        "exact_fixture",
        "adaptive_chebyshev_heat",
        json!({
            "selected_order": chebyshev.selected_order,
            "maximum_signal_error": chebyshev.maximum_signal_error,
            "verified_grid_error": chebyshev.verified_grid_error,
            "error_bound_kind": chebyshev.error_bound_kind,
        }),
    ));

    let wavelet = graph_wavelet_workflow(fixture(json!({
        "graph": base_graph(1.1),
        "scales": [1.0],
        "lowpass_scale": 1.0,
    }))?)?;
    let frame_weights = [0.0_f64, 1.0, 3.0]
        .map(|lambda| (-2.0 * lambda).exp() + (lambda * (-lambda).exp()).powi(2));
    let frame_lower = frame_weights.into_iter().fold(f64::INFINITY, f64::min);
    let frame_upper = frame_weights.into_iter().fold(0.0_f64, f64::max);
    require(
        frame_lower > 0.0
            && frame_upper.is_finite()
            && (wavelet.scales[0].energy - 2.0 * (-2.0_f64).exp()).abs() <= 1e-10,
        "spectral wavelet finite-graph frame oracle",
    )?;
    entries.push(passed(
        "spectral_wavelet_finite_frame",
        "exact_fixture",
        "spectral_graph_wavelet",
        json!({
            "finite_spectrum_frame_lower": frame_lower,
            "finite_spectrum_frame_upper": frame_upper,
            "lambda_one_signal_energy": wavelet.scales[0].energy,
        }),
    ));

    let diffusion = graph_diffusion_wavelet_workflow(fixture(json!({
        "graph": base_graph(1.1),
        "tolerance": 0.5,
        "maximum_levels": 4,
    }))?)?;
    require(
        diffusion.transform.reconstruction_max_abs_error <= 1e-10
            && diffusion.tree.levels.len() == 2
            && diffusion.tree.levels[0].retained_rank == 2
            && diffusion.tree.levels[1].retained_rank == 1,
        "diffusion wavelet reconstruction/compression oracle",
    )?;
    entries.push(passed(
        "diffusion_wavelet_reconstruction",
        "exact_fixture",
        "diffusion_wavelet",
        json!({
            "retained_ranks": diffusion.tree.levels.iter().map(|level| level.retained_rank).collect::<Vec<_>>(),
            "reconstruction_max_abs_error": diffusion.transform.reconstruction_max_abs_error,
        }),
    ));

    let scattering = graph_scattering_workflow(fixture(json!({
        "graph": base_graph(1.1),
        "scales": [1.0, 2.0],
        "maximum_order": 1,
        "stability_ratio_tolerance": 1.0,
        "perturbations": [{"id": "constant_shift", "signal_delta": [0.1, 0.1, 0.1]}],
    }))?)?;
    require(
        scattering.stability.iter().all(|result| result.passed),
        "scattering controlled signal stability oracle",
    )?;
    entries.push(passed(
        "scattering_controlled_signal_stability",
        "exact_fixture",
        "graph_scattering",
        json!({"stability": scattering.stability}),
    ));

    let hypergraph = hypergraph_signal_workflow(fixture(json!({
        "nodes": [
            {"id": "a", "signal": 1.0},
            {"id": "b", "signal": 0.0},
            {"id": "c", "signal": -1.0}
        ],
        "hyperedges": [
            {"id": "left", "hyperedge_type": "niche", "weight": 1.0, "members": [
                {"node_id": "a", "membership": 1.0}, {"node_id": "b", "membership": 1.0}
            ]},
            {"id": "right", "hyperedge_type": "niche", "weight": 1.0, "members": [
                {"node_id": "b", "membership": 1.0}, {"node_id": "c", "membership": 1.0}
            ]}
        ],
        "epsilon": 1e-12,
        "maximum_incidence_entries": 100
    }))?)?;
    require(
        hypergraph.node_degrees == [1.0, 2.0, 1.0]
            && hypergraph.hyperedge_degrees == [2.0, 2.0]
            && (hypergraph.laplacian[0][0] - 0.5).abs() <= 1e-12
            && (hypergraph.signal_smoothness - 0.5).abs() <= 1e-12,
        "hypergraph hand-incidence oracle",
    )?;
    entries.push(passed(
        "hypergraph_hand_incidence",
        "exact_fixture",
        "normalized_hypergraph_laplacian",
        json!({
            "node_degrees": hypergraph.node_degrees,
            "hyperedge_degrees": hypergraph.hyperedge_degrees,
            "signal_smoothness": hypergraph.signal_smoothness,
        }),
    ));

    let motif = typed_triangle_motif_workflow(fixture(json!({
        "nodes": [
            {"id": "a", "label": "cell", "stratum": "all"},
            {"id": "b", "label": "cell", "stratum": "all"},
            {"id": "c", "label": "vessel", "stratum": "all"}
        ],
        "edges": [
            {"source_id": "a", "target_id": "b"},
            {"source_id": "a", "target_id": "c"},
            {"source_id": "b", "target_id": "c"}
        ],
        "motif_id": "cell-cell-vessel-triangle",
        "required_labels": ["cell", "cell", "vessel"],
        "permutations": 39,
        "seed": 77,
        "maximum_triples": 100
    }))?)?;
    require(
        motif.observed_count == 1 && motif.triples_evaluated == 1,
        "motif exhaustive triangle oracle",
    )?;
    entries.push(passed(
        "motif_exhaustive_triangle",
        "exact_fixture",
        "typed_triangle_motif",
        json!({
            "observed_count": motif.observed_count,
            "triples_evaluated": motif.triples_evaluated,
            "motif_adjacency": motif.motif_adjacency,
        }),
    ));

    let hodge = simplicial_hodge_workflow(fixture(json!({
        "node_ids": ["a", "b", "c"],
        "edges": [
            {"source_id": "a", "target_id": "b", "flow": 1.0},
            {"source_id": "a", "target_id": "c", "flow": 2.0},
            {"source_id": "b", "target_id": "c", "flow": 3.0}
        ],
        "maximum_dimension": 2,
        "filter_step": 0.1,
        "orthogonality_tolerance": 1e-10,
        "maximum_clique_triples": 100
    }))?)?;
    require(
        hodge.boundary_of_boundary_max_abs <= 1e-12
            && hodge.decomposition.reconstruction_max_abs_error <= 1e-10
            && hodge.decomposition.gradient_curl_dot.abs() <= 1e-10
            && hodge.decomposition.gradient_harmonic_dot.abs() <= 1e-10
            && hodge.decomposition.curl_harmonic_dot.abs() <= 1e-10,
        "simplicial boundary/Hodge oracle",
    )?;
    entries.push(passed(
        "simplicial_boundary_and_hodge",
        "exact_fixture",
        "clique_complex_hodge",
        json!({
            "boundary_of_boundary_max_abs": hodge.boundary_of_boundary_max_abs,
            "reconstruction_max_abs_error": hodge.decomposition.reconstruction_max_abs_error,
            "orthogonality_dots": [hodge.decomposition.gradient_curl_dot, hodge.decomposition.gradient_harmonic_dot, hodge.decomposition.curl_harmonic_dot],
        }),
    ));

    let cellular = cellular_complex_workflow(fixture(cellular_fixture())?)?;
    require(
        cellular.baseline.boundary_of_boundary_max_abs <= 1e-12
            && cellular
                .perturbations
                .iter()
                .all(|item| item.topology_unchanged && item.boundary_matrices_unchanged),
        "cellular boundary/segmentation perturbation oracle",
    )?;
    entries.push(passed(
        "cellular_boundary_and_segmentation_perturbation",
        "exact_fixture",
        "cellular_complex",
        json!({
            "boundary_of_boundary_max_abs": cellular.baseline.boundary_of_boundary_max_abs,
            "perturbations": cellular.perturbations,
        }),
    ));

    let expanded = graph_spectral_workflow(fixture(base_graph(2.1))?)?;
    require(
        spectral.edges.len() == 2 && expanded.edges.len() == 3,
        "radius-choice sensitivity oracle",
    )?;
    entries.push(passed(
        "radius_choice_sensitivity",
        "scientific_stress",
        "canonical_graph",
        json!({
            "radius_1_1_edge_count": spectral.edges.len(),
            "radius_2_1_edge_count": expanded.edges.len(),
            "sensitivity_detected": true,
        }),
    ));
    entries.extend([
        unsupported(
            "knn_kernel_barrier_component_sensitivity",
            "canonical_graph",
            "canonical v1 supports only physical-radius, binary-weight, connected graphs",
        ),
        unsupported(
            "registration_perturbation",
            "canonical_graph",
            "no registration transform or uncertainty artifact is admitted by this synthetic suite",
        ),
        unsupported(
            "fixed_density_sparse_memory",
            "canonical_graph",
            "canonical v1 is deliberately bounded dense mathematics and makes no sparse-scale claim",
        ),
        GraphValidationEntry {
            id: "cpu_gpu_parity",
            category: "scientific_stress",
            algorithm: "all_graph_algorithms",
            status: "not_applicable",
            evidence: json!({"gpu_backend_supported": false}),
            limitation: Some("marklab-graph has no GPU backend"),
        },
    ]);

    let digest_bytes =
        serde_json::to_vec(&entries).map_err(|error| GraphError::Numerical(error.to_string()))?;
    Ok(GraphMathematicsValidationResult {
        format: "marklab.graph_mathematics_validation",
        version: 1,
        suite_digest: format!("{:x}", Sha256::digest(digest_bytes)),
        exact_fixture_status: "passed",
        scientific_stress_status: "partial_with_declared_unsupported_dimensions",
        entries,
        claim_status: "synthetic_exact_validation_only_no_pathology_performance_claim",
    })
}

fn fixture<T: DeserializeOwned>(value: Value) -> Result<T, GraphError> {
    serde_json::from_value(value).map_err(|error| GraphError::Numerical(error.to_string()))
}

fn require(condition: bool, oracle: &str) -> Result<(), GraphError> {
    if condition {
        Ok(())
    } else {
        Err(GraphError::Numerical(format!(
            "graph validation failed: {oracle}"
        )))
    }
}

fn passed(
    id: &'static str,
    category: &'static str,
    algorithm: &'static str,
    evidence: Value,
) -> GraphValidationEntry {
    GraphValidationEntry {
        id,
        category,
        algorithm,
        status: "passed",
        evidence,
        limitation: None,
    }
}

fn unsupported(
    id: &'static str,
    algorithm: &'static str,
    limitation: &'static str,
) -> GraphValidationEntry {
    GraphValidationEntry {
        id,
        category: "scientific_stress",
        algorithm,
        status: "not_supported",
        evidence: json!({}),
        limitation: Some(limitation),
    }
}

fn base_graph(radius_um: f64) -> Value {
    json!({
        "nodes": [
            {"id": "a", "coordinates_um": [0.0, 0.0], "signal": 1.0},
            {"id": "b", "coordinates_um": [1.0, 0.0], "signal": 0.0},
            {"id": "c", "coordinates_um": [2.0, 0.0], "signal": -1.0}
        ],
        "radius_um": radius_um,
        "weight": "binary",
        "laplacian": "combinatorial",
        "bands": [{"id": "all", "minimum": 0.0, "maximum": 3.5}],
        "maximum_pairs": 100
    })
}

fn cellular_fixture() -> Value {
    let interfaces = json!([
        {"id": "ab", "source_id": "a", "target_id": "b"},
        {"id": "bc", "source_id": "b", "target_id": "c"},
        {"id": "cd", "source_id": "c", "target_id": "d"},
        {"id": "da", "source_id": "d", "target_id": "a"}
    ]);
    let domains = json!([{"id": "tumor", "oriented_interfaces": [
        {"interface_id": "ab", "orientation": 1},
        {"interface_id": "bc", "orientation": 1},
        {"interface_id": "cd", "orientation": 1},
        {"interface_id": "da", "orientation": 1}
    ]}]);
    json!({
        "pathology_interpretation": "single annotated tumor compartment",
        "baseline": {
            "junctions": [
                {"id": "a", "coordinates_um": [0.0, 0.0]},
                {"id": "b", "coordinates_um": [1.0, 0.0]},
                {"id": "c", "coordinates_um": [1.0, 1.0]},
                {"id": "d", "coordinates_um": [0.0, 1.0]}
            ],
            "interfaces": interfaces.clone(),
            "domains": domains.clone()
        },
        "segmentation_perturbations": [{
            "id": "shifted_boundary",
            "segmentation": {
                "junctions": [
                    {"id": "a", "coordinates_um": [0.0, 0.0]},
                    {"id": "b", "coordinates_um": [1.1, 0.0]},
                    {"id": "c", "coordinates_um": [1.1, 1.0]},
                    {"id": "d", "coordinates_um": [0.0, 1.0]}
                ],
                "interfaces": interfaces,
                "domains": domains
            }
        }],
        "maximum_incidence_entries": 100
    })
}
