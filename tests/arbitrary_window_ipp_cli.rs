#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn weighted_arbitrary_window_likelihood_matches_constant_intensity_oracle_and_bounds_work() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let quadrature = directory.path().join("quadrature.csv");
    let window = directory.path().join("window.geojson");
    let output = directory.path().join("result.json");
    fs::write(
        &events,
        "event_id,x_um,y_um,covariate,offset\ne-1,0.25,0.25,0,0\ne-2,3.5,0.5,0,0\n",
    )
    .expect("events");
    fs::write(
        &quadrature,
        "node_id,x_um,y_um,weight_um2,covariate,offset\nq-1,0.25,0.25,1,0,0\nq-2,1.75,0.25,1,0,0\nq-3,0.25,1.75,1,0,0\nq-4,3.5,0.5,1,0,0\n",
    )
    .expect("quadrature");
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[2,0],[2,2],[0,2],[0,0]],[[0.5,0.5],[0.5,1.5],[1.5,1.5],[1.5,0.5],[0.5,0.5]]],[[[3,0],[4,0],[4,1],[3,1],[3,0]]]]}"#,
    )
    .expect("window");

    let mut command = Command::cargo_bin("marklab").expect("binary");
    command
        .args([
            "bayes",
            "arbitrary-window-ipp-likelihood",
            "--events",
            events.to_str().unwrap(),
            "--quadrature",
            quadrature.to_str().unwrap(),
            "--window",
            window.to_str().unwrap(),
            "--intercept",
            std::f64::consts::LN_2.to_string().as_str(),
            "--coefficient",
            "0",
            "--maximum-events",
            "2",
            "--maximum-quadrature-nodes",
            "4",
            "--maximum-work",
            "6",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("result JSON");
    assert_eq!(result["format"], "marklab.arbitrary_window_ipp_likelihood");
    assert_eq!(result["version"], 1);
    assert_eq!(result["window"]["area_um2"], 4.0);
    assert_eq!(result["window"]["component_count"], 2);
    assert_eq!(result["window"]["hole_count"], 1);
    assert_eq!(result["event_count"], 2);
    assert_eq!(result["quadrature_node_count"], 4);
    assert_eq!(result["quadrature_weight_um2"], 4.0);
    let event_term = result["event_term"].as_f64().unwrap();
    let integral_term = result["integral_term"].as_f64().unwrap();
    let log_likelihood = result["log_likelihood"].as_f64().unwrap();
    assert!((event_term - 2.0 * std::f64::consts::LN_2).abs() < 1e-12);
    assert!((integral_term - 8.0).abs() < 1e-12);
    assert!((log_likelihood - (2.0 * std::f64::consts::LN_2 - 8.0)).abs() < 1e-12);
    assert_eq!(result["statistical_unit"], "one_observed_point_pattern");
    assert_eq!(result["finite_result_policy"], "reject_non_finite");

    let mut limited = Command::cargo_bin("marklab").expect("binary");
    limited
        .args([
            "bayes",
            "arbitrary-window-ipp-likelihood",
            "--events",
            events.to_str().unwrap(),
            "--quadrature",
            quadrature.to_str().unwrap(),
            "--window",
            window.to_str().unwrap(),
            "--intercept",
            "0",
            "--coefficient",
            "0",
            "--maximum-events",
            "2",
            "--maximum-quadrature-nodes",
            "4",
            "--maximum-work",
            "5",
            "--out",
            directory.path().join("limited.json").to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("work exceeds"));
}
