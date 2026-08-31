#![cfg(feature = "cli")]

use std::fs;

#[path = "support/joint_location_embedding_fixture.rs"]
mod fixture;

#[test]
fn joint_location_embedding_fit_predicts_heldout_embeddings_from_observed_locations() {
    let directory = tempfile::tempdir().expect("tempdir");
    let location = directory.path().join("location.csv");
    let embedding = directory.path().join("embedding.csv");
    let output = directory.path().join("fit.json");
    fixture::write_inputs(&location, &embedding);

    fixture::command(&location, &embedding, &output)
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.joint_replicated_location_embedding_fit"
    );
    assert_eq!(result["statistical_unit"], "patient");
    assert_eq!(result["embedding_dimension"], 3);
    assert_eq!(result["factor_count"], 1);
    assert_eq!(result["training_pattern_count"], 8);
    assert_eq!(result["heldout_pattern_count"], 8);
    assert_eq!(
        result["holdout_policy"],
        "first_pattern_embeddings_train_second_pattern_embeddings_evaluate_locations_observed_for_both"
    );
    assert!(
        result["heldout_embedding_comparison"]["joint_minus_nonspatial"]["mean"]
            .as_f64()
            .unwrap()
            > 0.0,
        "{result}"
    );
    assert_eq!(
        result["embedding_projection_identity"],
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(result["backend"]["name"], "numpyro");
    assert!(matches!(
        result["fit_state"].as_str().unwrap(),
        "complete" | "nonconverged"
    ));
}
