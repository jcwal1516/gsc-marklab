#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn modular_joint_pathology_model_compiles_and_predicts_heldout_patient_outcome() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("joint.json");
    let latents = [-1.5_f64, -0.5, 0.5, 1.5];
    let patients = (0..4).map(|index| format!("p{index}")).collect::<Vec<_>>();
    let mut regions = Vec::new();
    let mut region_observations = Vec::new();
    for (patient, latent) in latents.iter().enumerate() {
        for replicate in 0..2 {
            let region_id = format!("p{patient}_r{replicate}");
            let local = latent + if replicate == 0 { -0.05 } else { 0.05 };
            regions.push(
                serde_json::json!({"region_id":region_id,"patient_id":format!("p{patient}")}),
            );
            region_observations.push(serde_json::json!({
                "region_id":region_id,
                "morphology":[local,0.5*local],
                "ihc":[1.2*local],
                "omics_counts":[(8.0*(0.35*local).exp()).round() as u64,(10.0*(-0.25*local).exp()).round() as u64],
                "library_size":1.0,
                "clone_label":if *latent > 0.0 {1} else {0}
            }));
        }
    }
    let outcomes = latents
        .iter()
        .enumerate()
        .map(|(index, latent)| {
            serde_json::json!({
                "patient_id":format!("p{index}"),
                "value":2.0*latent,
                "observed":index != 3
            })
        })
        .collect::<Vec<_>>();
    let fixture = serde_json::json!({
        "project_id":"synthetic_joint_pathology",
        "patients":patients,
        "regions":regions,
        "region_observations":region_observations,
        "patient_outcomes":outcomes,
        "model_spec":{
            "morphology":{"measurement_status":"measured","likelihood":"gaussian","feature_names":["m1","m2"]},
            "ihc":{"measurement_status":"measured","likelihood":"gaussian","feature_names":["i1"]},
            "omics":{"measurement_status":"measured","likelihood":"poisson","feature_names":["g1","g2"]},
            "clone":{"measurement_status":"measured","likelihood":"bernoulli","feature_names":["clone_positive"]},
            "clinical":{"measurement_status":"measured","likelihood":"gaussian","feature_names":["outcome"]}
        },
        "inference_plan":"laplace",
        "region_latent_standard_deviation":0.35,
        "gaussian_noise_standard_deviation":0.12,
        "parameter_precision":0.1,
        "maximum_iterations":1000,
        "seed":47,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "multimodal",
            "joint-pathology",
            "--input",
            input.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("JSON");
    assert_eq!(result["format"], "marklab.joint_pathology_model_fit");
    assert_eq!(result["model_ir"]["hierarchy"]["patient_count"], 4);
    assert_eq!(result["model_ir"]["hierarchy"]["region_count"], 8);
    assert_eq!(
        result["model_ir"]["observation_blocks"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    assert_eq!(
        result["heldout_outcome_predictions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(result["heldout_outcome_rmse"].as_f64().unwrap() < 0.6);
    assert!(
        result["diagnostics"]["objective_final"].as_f64().unwrap()
            < result["diagnostics"]["objective_initial"].as_f64().unwrap()
    );
    assert_eq!(result["fit_state"], "approximate_only");
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_joint_pathology_model"
    );
}
