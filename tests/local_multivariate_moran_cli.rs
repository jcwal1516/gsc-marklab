#![cfg(feature = "cli")]

use std::fs;

#[path = "support/local_multivariate_moran_fixture.rs"]
mod fixture;

#[test]
fn local_multivariate_moran_matches_hand_values_and_controls_the_location_family() {
    let root = tempfile::tempdir().expect("root");
    fixture::write_inputs(root.path());
    let output = root.path().join("result.json");

    fixture::command(root.path(), &output).assert().success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result bytes")).expect("result JSON");
    assert_eq!(result["format"], "marklab.local_multivariate_moran");
    assert_eq!(result["version"], 1);
    assert_eq!(result["statistical_unit"], "complete_multivariate_mark_row");
    assert_eq!(
        result["population_claim"],
        "within_specimen_field_diagnostic_only"
    );
    assert_eq!(
        result["null"],
        "complete_rows_randomly_labeled_within_declared_strata"
    );
    assert_eq!(
        result["multiplicity"],
        "single_step_max_abs_over_all_locations"
    );
    assert_eq!(result["point_count"], 4);
    assert_eq!(result["dimension"], 2);
    assert_eq!(result["directed_edge_count"], 6);
    assert_eq!(result["permutations_completed"], 31);
    assert_eq!(result["locations"][0]["point_id"], "p0");
    assert_eq!(result["locations"][1]["point_id"], "p1");
    assert_eq!(result["locations"][2]["point_id"], "p2");
    assert_eq!(result["locations"][3]["point_id"], "p3");
    for (row, expected) in result["locations"]
        .as_array()
        .expect("locations")
        .iter()
        .zip([1.0, 0.0, 0.0, 1.0])
    {
        assert!((row["statistic"].as_f64().expect("statistic") - expected).abs() < 1e-12);
        assert!(
            row["raw_p_value"].as_f64().expect("raw p")
                <= row["adjusted_p_value"].as_f64().expect("adjusted p")
        );
    }
    let global_p = result["global_max_abs_p_value"].as_f64().expect("global p");
    assert!((1.0 / 32.0..=1.0).contains(&global_p));
}

#[test]
fn canonical_single_slide_cellvit_projection_is_admitted_without_rewriting_values() {
    let root = tempfile::tempdir().expect("root");
    fixture::write_inputs(root.path());
    fs::write(
        root.path().join("points.csv"),
        "cell_id,x_um,y_um,cellvit_pc_000,cellvit_pc_001\n\
slide-a:p0,0,0,-1,-1\n\
slide-a:p1,1,0,-1,-1\n\
slide-a:p2,2,0,1,1\n\
slide-a:p3,3,0,1,1\n",
    )
    .expect("canonical CellViT source");
    let output = root.path().join("canonical.json");
    fixture::command(root.path(), &output).assert().success();
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("result JSON");
    assert_eq!(
        result["feature_names"],
        serde_json::json!(["cellvit_pc_000", "cellvit_pc_001"])
    );
    assert_eq!(result["stratum_count"], 1);
    assert_eq!(result["locations"][0]["point_id"], "slide-a:p0");
    assert_eq!(result["locations"][0]["statistic"], 1.0);
}
