#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn clone_phylogeography_and_niche_models_integrate_imported_uncertainty_and_patient_effects() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("clone_models.json");
    let mut cells = Vec::new();
    for patient in 0..4 {
        let patient_offset = (patient as f64 - 1.5) * 0.1;
        for index in 0..20 {
            let clone_a = index < 10;
            cells.push(serde_json::json!({
                "cell_id":format!("p{patient}_c{index:02}"),"patient_id":format!("p{patient}"),
                "coordinates_um":[index as f64,patient as f64],
                "clone_probabilities":if clone_a {[0.9,0.1]} else {[0.1,0.9]},
                "neighborhood_features":[if clone_a {1.0+patient_offset} else {-1.0+patient_offset}]
            }));
        }
    }
    let fixture = serde_json::json!({
        "tree":{"tree_id":"imported_tree_v1","root_id":"root","nodes":["root","A","B"],"branches":[
            {"parent":"root","child":"A","length":1.0},{"parent":"root","child":"B","length":1.0}
        ]},
        "clone_labels":["A","B"],
        "clone_locations":[
            {"clone_id":"root","mean_xy_um":[0.0,0.0],"covariance_um2":[[0.01,0.0],[0.0,0.01]]},
            {"clone_id":"A","mean_xy_um":[1.0,0.2],"covariance_um2":[[0.04,0.0],[0.0,0.04]]},
            {"clone_id":"B","mean_xy_um":[-1.0,-0.2],"covariance_um2":[[0.04,0.0],[0.0,0.04]]}
        ],
        "cells":cells,
        "diffusion_prior":{"inverse_gamma_shape":2.0,"inverse_gamma_scale":0.2},
        "posterior_draws":256,
        "seed":101,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");
    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "spatial3d",
            "clone-models",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(
        result["format"],
        "marklab.clone_phylogeography_and_niche_models"
    );
    assert!(
        result["phylogeography"]["diffusion_posterior_mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(
        result["phylogeography"]["location_uncertainty_integrated"],
        true
    );
    assert!(
        (result["niche_model"]["clone_a_minus_b_contrast"][0]
            .as_f64()
            .unwrap()
            - 2.0)
            .abs()
            < 0.2
    );
    assert!(
        result["niche_model"]["clone_a_minus_b_standard_error"][0]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(result["niche_model"]["patient_count"], 4);
    assert!(
        result["niche_model"]["leave_one_patient_out_maximum_contrast_change"][0]
            .as_f64()
            .unwrap()
            < 0.2
    );
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_clone_models_no_identified_history"
    );
}
