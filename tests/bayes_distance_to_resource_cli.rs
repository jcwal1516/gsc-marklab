#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn exact_hierarchical_spline_recovers_a_synthetic_resource_response() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    let patient_offsets = [-0.3, -0.1, 0.1, 0.3];
    let mut observations = Vec::new();
    for (patient_index, patient_offset) in patient_offsets.into_iter().enumerate() {
        for distance in 0_u32..=4 {
            let compartment = if (distance as usize + patient_index).is_multiple_of(2) {
                "a"
            } else {
                "b"
            };
            let resource_density = f64::from(((distance as usize + patient_index) % 3) as u32);
            let accessibility =
                f64::from(((2 * distance as usize + patient_index) % 4) as u32) / 3.0;
            let outcome = 1.0
                + 2.0 * f64::from(distance)
                + 3.0 * (f64::from(distance) - 2.0).max(0.0)
                + if compartment == "b" { 0.5 } else { 0.0 }
                + 0.25 * resource_density
                - 0.4 * accessibility
                + patient_offset;
            observations.push(serde_json::json!({
                "cell_id": format!("c{patient_index}-{distance}"),
                "patient_id": format!("p{patient_index}"),
                "x_um": distance,
                "y_um": patient_index + 1,
                "outcome": outcome,
                "compartment": compartment,
                "resource_density": resource_density,
                "accessibility": accessibility
            }));
        }
    }
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "resources": [{
                "resource_id": "vessel-1",
                "start_x_um": 0.0,
                "start_y_um": 0.0,
                "end_x_um": 0.0,
                "end_y_um": 10.0
            }],
            "spline_knots_um": [2.0],
            "observations": observations
        }))
        .unwrap(),
    )
    .unwrap();
    let output = directory.path().join("result.json");
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "distance-to-resource",
            "--input",
            input.to_str().unwrap(),
            "--coefficient-prior-sd",
            "10",
            "--patient-effect-prior-sd",
            "1",
            "--known-noise-sd",
            "0.05",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.distance_to_resource_fit");
    assert_eq!(result["model"]["distance_basis"], "linear_hinge_spline");
    assert_eq!(result["model"]["hierarchy"], serde_json::json!(["patient"]));
    for row in result["distances"].as_array().unwrap() {
        let expected = row["cell_id"]
            .as_str()
            .unwrap()
            .split('-')
            .nth(1)
            .unwrap()
            .parse::<f64>()
            .unwrap();
        assert!((row["unsigned_distance_um"].as_f64().unwrap() - expected).abs() < 1e-12);
    }
    let coefficients = result["posterior"]["fixed_coefficients"]
        .as_array()
        .unwrap();
    let coefficient = |name: &str| {
        coefficients.iter().find(|row| row["name"] == name).unwrap()["mean"]
            .as_f64()
            .unwrap()
    };
    assert!((coefficient("distance_um") - 2.0).abs() < 0.1);
    assert!((coefficient("distance_hinge_2_um") - 3.0).abs() < 0.1);
    assert!(result["posterior_predictive"]["rmse"].as_f64().unwrap() < 0.05);
    assert_eq!(
        result["posterior_predictive"]["patient_checks"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        result["posterior_predictive"]["resource_checks"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        result["claim_status"],
        "association_not_transport_or_resource_causality"
    );
}
