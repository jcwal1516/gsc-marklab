#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn biological_similarity_atlas_maps_queries_and_validates_leave_one_patient_out() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("atlas.json");
    let mut reference = Vec::new();
    for patient in 0..6 {
        let offset = (patient as f64 - 2.5) * 0.03;
        reference.push(serde_json::json!({
            "sample_id":format!("p{patient}_a"),"patient_id":format!("p{patient}"),
            "site_id":if patient < 3 {"site1"} else {"site2"},"domain":"A",
            "features":[-2.0+offset,-1.0-offset]
        }));
        reference.push(serde_json::json!({
            "sample_id":format!("p{patient}_b"),"patient_id":format!("p{patient}"),
            "site_id":if patient < 3 {"site1"} else {"site2"},"domain":"B",
            "features":[2.0+offset,1.0-offset]
        }));
    }
    let fixture = serde_json::json!({
        "atlas_id":"synthetic_domains",
        "representation":{"modality":"measured_region_features","model_version":"oracle-v1","stain":"synthetic","feature_names":["f1","f2"]},
        "alignment":{"method":"biological_similarity_no_physical_registration","distance":"shrinkage_mahalanobis","ood_distance_threshold":4.0},
        "reference_samples":reference,
        "queries":[
            {"query_id":"qa","features":[-2.1,-0.9],"expected_domain":"A"},
            {"query_id":"qb","features":[2.1,0.9],"expected_domain":"B"},
            {"query_id":"novel","features":[12.0,12.0],"expected_domain":null}
        ],
        "perturbations":[
            {"perturbation_id":"none","feature_shift":[0.0,0.0],"subsample_fraction":1.0},
            {"perturbation_id":"scanner_shift","feature_shift":[0.15,-0.1],"subsample_fraction":0.75}
        ],
        "covariance_ridge":0.05,
        "seed":71,
        "timeout_seconds":60
    });
    fs::write(&input, serde_json::to_vec_pretty(&fixture).unwrap()).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "registration",
            "atlas",
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
        "marklab.spatial_atlas_mapping_and_validation"
    );
    assert_eq!(result["atlas"]["support_type"], "domain_prototypes");
    assert_eq!(result["atlas"]["physical_registration_claim"], false);
    assert_eq!(result["validation"]["leave_one_patient_out_accuracy"], 1.0);
    assert_eq!(
        result["validation"]["perturbations"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let mappings = result["query_mappings"].as_array().unwrap();
    assert!(mappings[0]["domain_probabilities"]["A"].as_f64().unwrap() > 0.95);
    assert!(mappings[1]["domain_probabilities"]["B"].as_f64().unwrap() > 0.95);
    assert!(mappings[2]["unmatched_probability"].as_f64().unwrap() > 0.5);
    assert_eq!(
        result["claim_status"],
        "experimental_synthetic_biological_similarity_atlas"
    );
}
