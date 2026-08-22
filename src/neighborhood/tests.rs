use crate::{
    multimodal::cell_table::{CellSection, FusedCell},
    neighborhood::{
        enrichment::{edge_enrichment, edge_enrichment_with_strata, LabelPair},
        graph::{build_spatial_graph, GraphConfig, SpatialEdge, SpatialGraph},
    },
    EnrichmentStatisticUnavailableReason, NeighborhoodEnrichmentResult,
};

fn cell(id: &str, x: f64, y: f64, label: &str) -> FusedCell {
    FusedCell {
        source_section: CellSection::He,
        source_cell_id: id.into(),
        x_um_registered: x,
        y_um_registered: y,
        mmr_mark: None,
        mmr_probability: None,
        cell_type: Some(label.into()),
        cell_type_probability: Some(1.0),
        same_section: false,
        registration_error_um: Some(5.0),
        timepoint: "pre".into(),
        case_id: "case1".into(),
        protein: "MSH6".into(),
    }
}

fn cell_with_registration_error(
    id: &str,
    x: f64,
    y: f64,
    registration_error_um: Option<f64>,
) -> FusedCell {
    FusedCell {
        registration_error_um,
        ..cell(id, x, y, "tumor")
    }
}

fn ihc_cell(id: &str, x: f64, y: f64, mmr_mark: u8, cell_type: Option<&str>) -> FusedCell {
    FusedCell {
        source_section: CellSection::Ihc,
        source_cell_id: id.into(),
        x_um_registered: x,
        y_um_registered: y,
        mmr_mark: Some(mmr_mark),
        mmr_probability: Some(1.0),
        cell_type: cell_type.map(str::to_owned),
        cell_type_probability: None,
        same_section: false,
        registration_error_um: Some(5.0),
        timepoint: "pre".into(),
        case_id: "case1".into(),
        protein: "MSH6".into(),
    }
}

fn ihc_probability_cell(id: &str, x: f64, y: f64, mmr_probability: f64) -> FusedCell {
    FusedCell {
        source_section: CellSection::Ihc,
        source_cell_id: id.into(),
        x_um_registered: x,
        y_um_registered: y,
        mmr_mark: None,
        mmr_probability: Some(mmr_probability),
        cell_type: None,
        cell_type_probability: None,
        same_section: false,
        registration_error_um: Some(5.0),
        timepoint: "pre".into(),
        case_id: "case1".into(),
        protein: "MSH6".into(),
    }
}

fn edge(source: usize, target: usize) -> SpatialEdge {
    SpatialEdge {
        source,
        target,
        distance_um: 1.0,
        angle_rad: 0.0,
        below_registration_resolution: false,
    }
}

fn sparse_positive_enrichment() -> NeighborhoodEnrichmentResult {
    let cells = vec![
        cell("a", 0.0, 0.0, "mmr_abnormal"),
        cell("b", 1.0, 0.0, "mmr_abnormal"),
        cell("c", 10.0, 0.0, "lymphocyte"),
        cell("d", 11.0, 0.0, "lymphocyte"),
    ];
    let graph = SpatialGraph {
        n_nodes: cells.len(),
        edges: vec![edge(0, 1)],
    };
    let pair = [LabelPair::new("mmr_abnormal", "mmr_abnormal")];

    (0..1_024)
        .find_map(|seed| {
            let row = edge_enrichment(&cells, &graph, &pair, 1, seed)
                .expect("sparse enrichment")
                .remove(0);
            (row.observed_edges > 0 && row.expected_edges == 0.0).then_some(row)
        })
        .expect("a deterministic sparse null with zero expected edges")
}

#[test]
fn radius_graph_connects_only_cells_within_radius() {
    let cells = vec![
        cell("a", 0.0, 0.0, "tumor"),
        cell("b", 3.0, 4.0, "lymphocyte"),
        cell("c", 20.0, 0.0, "tumor"),
    ];
    let graph = build_spatial_graph(
        &cells,
        GraphConfig {
            radius_um: Some(5.1),
            k_nearest: None,
        },
    )
    .expect("graph");

    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].source, 0);
    assert_eq!(graph.edges[0].target, 1);
    assert!((graph.edges[0].distance_um - 5.0).abs() < 1.0e-9);
}

#[test]
fn enrichment_detects_observed_label_pair_edges() {
    let cells = vec![
        cell("a", 0.0, 0.0, "mmr_abnormal"),
        cell("b", 1.0, 0.0, "mmr_abnormal"),
        cell("c", 10.0, 0.0, "lymphocyte"),
        cell("d", 11.0, 0.0, "lymphocyte"),
    ];
    let graph = build_spatial_graph(
        &cells,
        GraphConfig {
            radius_um: Some(2.0),
            k_nearest: None,
        },
    )
    .expect("graph");

    let rows: Vec<NeighborhoodEnrichmentResult> = edge_enrichment(
        &cells,
        &graph,
        &[LabelPair::new("mmr_abnormal", "mmr_abnormal")],
        19,
        123,
    )
    .expect("enrichment");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].label_a, "mmr_abnormal");
    assert_eq!(rows[0].label_b, "mmr_abnormal");
    assert_eq!(rows[0].observed_edges, 1);
    assert!(rows[0].p_value.is_some());
}

#[test]
fn remediation_sparse_enrichment_statistics_are_finite_or_typed_undefined() {
    let row = sparse_positive_enrichment();

    assert_eq!(row.enrichment_ratio, None);
    assert_eq!(
        row.enrichment_ratio_unavailable_reason,
        Some(EnrichmentStatisticUnavailableReason::ZeroExpectedEdges)
    );
    assert_eq!(row.z_score, None);
    assert_eq!(
        row.z_score_unavailable_reason,
        Some(EnrichmentStatisticUnavailableReason::InsufficientNullSamples)
    );
    assert!(row.p_value.is_some());
}

#[test]
fn remediation_sparse_enrichment_roundtrips_through_json() {
    let row = sparse_positive_enrichment();
    let json = serde_json::to_string(&row).expect("serialize sparse enrichment");
    let roundtrip: NeighborhoodEnrichmentResult =
        serde_json::from_str(&json).expect("deserialize sparse enrichment");

    assert_eq!(roundtrip, row);
}

#[test]
fn enrichment_counts_mixed_label_pairs_as_undirected() {
    let cells = vec![
        cell("a", 0.0, 0.0, "mmr_abnormal"),
        cell("b", 1.0, 0.0, "lymphocyte"),
    ];
    let graph = build_spatial_graph(
        &cells,
        GraphConfig {
            radius_um: Some(2.0),
            k_nearest: None,
        },
    )
    .expect("graph");

    let rows = edge_enrichment(
        &cells,
        &graph,
        &[LabelPair::new("lymphocyte", "mmr_abnormal")],
        19,
        123,
    )
    .expect("enrichment");

    assert_eq!(rows[0].label_a, "lymphocyte");
    assert_eq!(rows[0].label_b, "mmr_abnormal");
    assert_eq!(rows[0].observed_edges, 1);
}

#[test]
fn enrichment_prefers_ihc_mmr_mark_over_cell_type() {
    let cells = vec![
        ihc_cell("a", 0.0, 0.0, 1, Some("lymphocyte")),
        cell("b", 1.0, 0.0, "mmr_abnormal"),
    ];
    let graph = build_spatial_graph(
        &cells,
        GraphConfig {
            radius_um: Some(2.0),
            k_nearest: None,
        },
    )
    .expect("graph");

    let rows = edge_enrichment(
        &cells,
        &graph,
        &[LabelPair::new("mmr_abnormal", "mmr_abnormal")],
        19,
        123,
    )
    .expect("enrichment");

    assert_eq!(rows[0].observed_edges, 1);
}

#[test]
fn enrichment_maps_probability_only_ihc_labels() {
    let cells = vec![
        ihc_probability_cell("a", 0.0, 0.0, 0.75),
        ihc_probability_cell("b", 1.0, 0.0, 0.25),
    ];
    let graph = build_spatial_graph(
        &cells,
        GraphConfig {
            radius_um: Some(2.0),
            k_nearest: None,
        },
    )
    .expect("graph");

    let rows = edge_enrichment(
        &cells,
        &graph,
        &[LabelPair::new("mmr_abnormal", "mmr_retained")],
        19,
        123,
    )
    .expect("enrichment");

    assert_eq!(rows[0].observed_edges, 1);
}

#[test]
fn enrichment_null_preserves_source_section_labels() {
    let cells = vec![
        ihc_cell("ihc-a", 0.0, 0.0, 1, None),
        ihc_cell("ihc-b", 1.0, 0.0, 0, None),
        cell("he-a", 10.0, 0.0, "lymphocyte"),
        cell("he-b", 11.0, 0.0, "stroma"),
    ];
    let graph = SpatialGraph {
        n_nodes: cells.len(),
        edges: vec![edge(0, 1), edge(2, 3)],
    };

    let rows = edge_enrichment(
        &cells,
        &graph,
        &[LabelPair::new("mmr_abnormal", "lymphocyte")],
        31,
        123,
    )
    .expect("enrichment");

    assert_eq!(rows[0].observed_edges, 0);
    assert_eq!(rows[0].expected_edges, 0.0);
    assert_eq!(rows[0].enrichment_ratio, None);
    assert_eq!(rows[0].z_score, None);
    assert_eq!(
        rows[0].z_score_unavailable_reason,
        Some(EnrichmentStatisticUnavailableReason::ZeroNullVariance)
    );
}

#[test]
fn enrichment_is_deterministic_for_same_seed() {
    let cells = vec![
        cell("a", 0.0, 0.0, "mmr_abnormal"),
        cell("b", 1.0, 0.0, "lymphocyte"),
        cell("c", 2.0, 0.0, "mmr_abnormal"),
        cell("d", 3.0, 0.0, "lymphocyte"),
    ];
    let graph = build_spatial_graph(
        &cells,
        GraphConfig {
            radius_um: Some(1.1),
            k_nearest: None,
        },
    )
    .expect("graph");
    let pairs = [
        LabelPair::new("mmr_abnormal", "lymphocyte"),
        LabelPair::new("mmr_abnormal", "mmr_abnormal"),
    ];

    let first = edge_enrichment(&cells, &graph, &pairs, 31, 777).expect("first enrichment");
    let second = edge_enrichment(&cells, &graph, &pairs, 31, 777).expect("second enrichment");

    assert_eq!(first, second);
}

#[test]
fn enrichment_sets_bh_q_values_for_multiple_pairs() {
    let cells = vec![
        cell("a", 0.0, 0.0, "mmr_abnormal"),
        cell("b", 1.0, 0.0, "mmr_abnormal"),
        cell("c", 2.0, 0.0, "lymphocyte"),
        cell("d", 3.0, 0.0, "tumor"),
    ];
    let graph = build_spatial_graph(
        &cells,
        GraphConfig {
            radius_um: Some(1.1),
            k_nearest: None,
        },
    )
    .expect("graph");

    let rows = edge_enrichment(
        &cells,
        &graph,
        &[
            LabelPair::new("mmr_abnormal", "mmr_abnormal"),
            LabelPair::new("mmr_abnormal", "lymphocyte"),
            LabelPair::new("lymphocyte", "tumor"),
        ],
        31,
        123,
    )
    .expect("enrichment");

    assert!(rows
        .iter()
        .all(|row| row.q_value.is_some_and(|q_value| q_value <= 1.0)));

    let mut sorted: Vec<_> = rows
        .iter()
        .map(|row| (row.p_value.expect("p-value"), row.q_value.expect("q-value")))
        .collect();
    sorted.sort_by(|left, right| left.0.total_cmp(&right.0));
    for window in sorted.windows(2) {
        assert!(window[0].1 <= window[1].1);
    }
}

#[test]
fn enrichment_rejects_graph_cell_mismatch() {
    let cells = vec![cell("a", 0.0, 0.0, "tumor")];
    let graph = SpatialGraph {
        n_nodes: 2,
        edges: Vec::new(),
    };

    let err = edge_enrichment(&cells, &graph, &[LabelPair::new("tumor", "tumor")], 1, 123)
        .expect_err("mismatched graph should fail");

    assert!(err.to_string().contains("node count"));
}

#[test]
fn enrichment_rejects_malformed_public_graph_edges() {
    let cells = vec![cell("a", 0.0, 0.0, "tumor"), cell("b", 1.0, 0.0, "tumor")];

    let self_edge_graph = SpatialGraph {
        n_nodes: 2,
        edges: vec![edge(0, 0)],
    };
    let err = edge_enrichment(
        &cells,
        &self_edge_graph,
        &[LabelPair::new("tumor", "tumor")],
        1,
        123,
    )
    .expect_err("self edge should fail");
    assert!(err.to_string().contains("self-edge"));

    let mirrored_edge_graph = SpatialGraph {
        n_nodes: 2,
        edges: vec![edge(0, 1), edge(1, 0)],
    };
    let err = edge_enrichment(
        &cells,
        &mirrored_edge_graph,
        &[LabelPair::new("tumor", "tumor")],
        1,
        123,
    )
    .expect_err("mirrored edges should fail");
    assert!(err.to_string().contains("duplicate or mirrored"));
}

#[test]
fn enrichment_rejects_zero_permutations() {
    let cells = vec![cell("a", 0.0, 0.0, "tumor")];
    let graph = SpatialGraph {
        n_nodes: 1,
        edges: Vec::new(),
    };

    let err = edge_enrichment(&cells, &graph, &[LabelPair::new("tumor", "tumor")], 0, 123)
        .expect_err("zero permutations should fail");

    assert!(err
        .to_string()
        .contains("permutations must be greater than zero"));
}

#[test]
fn enrichment_rejects_blank_label_pair_labels() {
    let cells = vec![cell("a", 0.0, 0.0, "tumor")];
    let graph = SpatialGraph {
        n_nodes: 1,
        edges: Vec::new(),
    };

    let err = edge_enrichment(&cells, &graph, &[LabelPair::new("tumor", "  ")], 1, 123)
        .expect_err("blank label should fail");

    assert!(err
        .to_string()
        .contains("label pair labels must be non-empty"));
}

#[test]
fn enrichment_allows_empty_label_pairs_as_noop() {
    let cells = vec![cell("a", 0.0, 0.0, "tumor")];
    let graph = SpatialGraph {
        n_nodes: 1,
        edges: Vec::new(),
    };

    let rows = edge_enrichment(&cells, &graph, &[], 1, 123).expect("empty label pairs");

    assert!(rows.is_empty());
}

#[test]
fn enrichment_strategies_preserve_deterministic_reference_outputs() {
    let cells = vec![
        ihc_cell("ihc-a", 0.0, 0.0, 1, None),
        ihc_cell("ihc-b", 1.0, 0.0, 0, None),
        ihc_cell("ihc-c", 2.0, 0.0, 1, None),
        cell("he-a", 0.0, 1.0, "lymphocyte"),
        cell("he-b", 1.0, 1.0, "stroma"),
        cell("he-c", 2.0, 1.0, "lymphocyte"),
    ];
    let graph = SpatialGraph {
        n_nodes: cells.len(),
        edges: vec![
            edge(0, 1),
            edge(0, 3),
            edge(1, 2),
            edge(1, 4),
            edge(2, 5),
            edge(3, 4),
            edge(4, 5),
        ],
    };
    let pairs = vec![
        LabelPair::new("mmr_abnormal", "lymphocyte"),
        LabelPair::new("lymphocyte", "stroma"),
    ];
    let strata = vec![
        "ihc-left".into(),
        "ihc-center".into(),
        "ihc-left".into(),
        "he-edge".into(),
        "he-center".into(),
        "he-edge".into(),
    ];

    let unstratified = edge_enrichment(&cells, &graph, &pairs, 19, 77).expect("unstratified");
    let stratified =
        edge_enrichment_with_strata(&cells, &graph, &pairs, 19, 77, &strata).expect("stratified");

    assert_eq!(
        serde_json::to_value(unstratified).unwrap(),
        serde_json::json!([
            {
                "label_a": "lymphocyte",
                "label_b": "mmr_abnormal",
                "observed_edges": 2,
                "expected_edges": 1.1578947368421053,
                "enrichment_ratio": 1.7272727272727273,
                "enrichment_ratio_unavailable_reason": null,
                "z_score": 2.2478059477960652,
                "z_score_unavailable_reason": null,
                "p_value": 0.2,
                "q_value": 0.4
            },
            {
                "label_a": "lymphocyte",
                "label_b": "stroma",
                "observed_edges": 2,
                "expected_edges": 1.4210526315789473,
                "enrichment_ratio": 1.4074074074074074,
                "enrichment_ratio_unavailable_reason": null,
                "z_score": 1.141328865379023,
                "z_score_unavailable_reason": null,
                "p_value": 0.45,
                "q_value": 0.45
            }
        ])
    );
    assert_eq!(
        serde_json::to_value(stratified).unwrap(),
        serde_json::json!([
            {
                "label_a": "lymphocyte",
                "label_b": "mmr_abnormal",
                "observed_edges": 2,
                "expected_edges": 2.0,
                "enrichment_ratio": 1.0,
                "enrichment_ratio_unavailable_reason": null,
                "z_score": null,
                "z_score_unavailable_reason": "zero_null_variance",
                "p_value": 1.0,
                "q_value": 1.0
            },
            {
                "label_a": "lymphocyte",
                "label_b": "stroma",
                "observed_edges": 2,
                "expected_edges": 2.0,
                "enrichment_ratio": 1.0,
                "enrichment_ratio_unavailable_reason": null,
                "z_score": null,
                "z_score_unavailable_reason": "zero_null_variance",
                "p_value": 1.0,
                "q_value": 1.0
            }
        ])
    );
}

#[test]
fn stratified_enrichment_rejects_mismatched_strata() {
    let cells = vec![
        cell("a", 0.0, 0.0, "lymphocyte"),
        cell("b", 1.0, 0.0, "stroma"),
    ];
    let graph = SpatialGraph {
        n_nodes: cells.len(),
        edges: vec![edge(0, 1)],
    };

    let error = edge_enrichment_with_strata(
        &cells,
        &graph,
        &[LabelPair::new("lymphocyte", "stroma")],
        19,
        77,
        &["only-one".into()],
    )
    .expect_err("mismatched strata");

    assert!(error
        .to_string()
        .contains("null-model stratum count 1 does not match cell count 2"));
}

#[test]
fn graph_config_rejects_missing_radius_and_knn() {
    let err = build_spatial_graph(
        &[cell("a", 0.0, 0.0, "tumor")],
        GraphConfig {
            radius_um: None,
            k_nearest: None,
        },
    )
    .expect_err("empty config should fail");

    assert!(err.to_string().contains("requires radius_um or k_nearest"));
}

#[test]
fn graph_config_rejects_non_positive_radius() {
    let err = build_spatial_graph(
        &[cell("a", 0.0, 0.0, "tumor")],
        GraphConfig {
            radius_um: Some(0.0),
            k_nearest: None,
        },
    )
    .expect_err("zero radius should fail");

    assert!(err
        .to_string()
        .contains("radius_um must be finite and positive"));
}

#[test]
fn graph_config_rejects_zero_knn() {
    let err = build_spatial_graph(
        &[cell("a", 0.0, 0.0, "tumor")],
        GraphConfig {
            radius_um: None,
            k_nearest: Some(0),
        },
    )
    .expect_err("zero kNN should fail");

    assert!(err
        .to_string()
        .contains("k_nearest must be positive when configured"));
}

#[test]
fn graph_config_rejects_non_finite_radius() {
    for radius_um in [f64::NAN, f64::INFINITY] {
        let err = build_spatial_graph(
            &[cell("a", 0.0, 0.0, "tumor")],
            GraphConfig {
                radius_um: Some(radius_um),
                k_nearest: None,
            },
        )
        .expect_err("non-finite radius should fail");

        assert!(err
            .to_string()
            .contains("radius_um must be finite and positive"));
    }
}

#[test]
fn graph_rejects_non_finite_registered_coordinates() {
    for (x_um, y_um) in [(f64::NAN, 0.0), (0.0, f64::INFINITY)] {
        let err = build_spatial_graph(
            &[cell("a", x_um, y_um, "tumor")],
            GraphConfig {
                radius_um: Some(1.0),
                k_nearest: None,
            },
        )
        .expect_err("non-finite coordinate should fail");

        assert!(err
            .to_string()
            .contains("registered coordinates must be finite"));
    }
}

#[test]
fn graph_rejects_invalid_registration_error() {
    for registration_error_um in [Some(-1.0), Some(f64::NAN), Some(f64::INFINITY)] {
        let err = build_spatial_graph(
            &[cell_with_registration_error(
                "a",
                0.0,
                0.0,
                registration_error_um,
            )],
            GraphConfig {
                radius_um: Some(1.0),
                k_nearest: None,
            },
        )
        .expect_err("invalid registration error should fail");

        assert!(err
            .to_string()
            .contains("registration_error_um must be finite and non-negative"));
    }
}

#[test]
fn knn_graph_is_deterministic_and_stores_undirected_edges_once() {
    let cells = vec![
        cell("a", 0.0, 0.0, "tumor"),
        cell("b", 1.0, 0.0, "lymphocyte"),
        cell("c", 2.0, 0.0, "tumor"),
    ];
    let graph = build_spatial_graph(
        &cells,
        GraphConfig {
            radius_um: None,
            k_nearest: Some(1),
        },
    )
    .expect("graph");

    let edges: Vec<_> = graph
        .edges
        .iter()
        .map(|edge| (edge.source, edge.target))
        .collect();
    assert_eq!(edges, vec![(0, 1), (1, 2)]);
}

#[test]
fn knn_ties_choose_lower_target_index_first() {
    let cells = vec![
        cell("a", 0.0, 0.0, "tumor"),
        cell("b", -1.0, 0.0, "lymphocyte"),
        cell("c", 1.0, 0.0, "tumor"),
    ];
    let graph = build_spatial_graph(
        &cells,
        GraphConfig {
            radius_um: None,
            k_nearest: Some(1),
        },
    )
    .expect("graph");

    let edges: Vec<_> = graph
        .edges
        .iter()
        .map(|edge| (edge.source, edge.target))
        .collect();
    assert_eq!(edges, vec![(0, 1), (0, 2)]);
}

#[test]
fn duplicate_coordinates_produce_deterministic_zero_distance_edge() {
    let cells = vec![
        cell("a", 0.0, 0.0, "tumor"),
        cell("b", 0.0, 0.0, "lymphocyte"),
        cell("c", 2.0, 0.0, "tumor"),
    ];
    let graph = build_spatial_graph(
        &cells,
        GraphConfig {
            radius_um: None,
            k_nearest: Some(1),
        },
    )
    .expect("graph");

    assert_eq!(graph.edges[0].source, 0);
    assert_eq!(graph.edges[0].target, 1);
    assert_eq!(graph.edges[0].distance_um, 0.0);
}

#[test]
fn registration_resolution_flag_uses_strict_boundary() {
    let cases = [
        (4.999, Some(2.5), true),
        (5.0, Some(2.5), false),
        (5.001, Some(2.5), false),
        (0.0, None, false),
    ];

    for (distance_um, registration_error_um, expected) in cases {
        let cells = vec![
            cell_with_registration_error("a", 0.0, 0.0, registration_error_um),
            cell_with_registration_error("b", distance_um, 0.0, registration_error_um),
        ];
        let graph = build_spatial_graph(
            &cells,
            GraphConfig {
                radius_um: Some(distance_um.max(1.0)),
                k_nearest: None,
            },
        )
        .expect("graph");

        assert_eq!(graph.edges[0].below_registration_resolution, expected);
    }
}
