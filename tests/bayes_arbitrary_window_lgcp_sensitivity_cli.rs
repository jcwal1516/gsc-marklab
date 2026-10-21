#![cfg(feature = "cli")]

use std::{collections::HashSet, fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn arbitrary_window_lgcp_runs_fixed_amplitude_and_length_scale_grid() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let membership = directory.path().join("membership.csv");
    let quadrature = directory.path().join("quadrature.csv");
    let window = directory.path().join("window.geojson");
    let output = directory.path().join("sensitivity.json");
    let mut event_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    let mut membership_csv = String::from("event_id,quadrature_node_id\n");
    for (index, count) in [2, 4, 8, 16].into_iter().enumerate() {
        let ix = index % 2;
        let iy = index / 2;
        let covariate = ix as f64 * 2.0 - 1.0;
        for event in 0..count {
            let id = format!("c{index}-e{event:02}");
            writeln!(
                event_csv,
                "{id},{},{},{covariate},0",
                ix as f64 + 0.5,
                iy as f64 + 0.5
            )
            .unwrap();
            writeln!(membership_csv, "{id},q-{index}").unwrap();
        }
    }
    fs::write(&events, event_csv).unwrap();
    fs::write(&membership, membership_csv).unwrap();
    fs::write(
        &quadrature,
        "node_id,x_um,y_um,weight_um2,covariate,offset\nq-0,0.5,0.5,1,-1,0\nq-1,1.5,0.5,1,1,0\nq-2,0.5,1.5,1,-1,0\nq-3,1.5,1.5,1,1,0\n",
    )
    .unwrap();
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[2,0],[2,2],[0,2],[0,0]]]]}"#,
    )
    .unwrap();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "arbitrary-window-lgcp-sensitivity",
            "--events",
            events.to_str().unwrap(),
            "--event-membership",
            membership.to_str().unwrap(),
            "--quadrature",
            quadrature.to_str().unwrap(),
            "--window",
            window.to_str().unwrap(),
            "--intercept-prior-mean",
            "1",
            "--intercept-prior-sd",
            "1",
            "--coefficient-prior-mean",
            "0",
            "--coefficient-prior-sd",
            "1",
            "--field-amplitude",
            "0.2",
            "--field-length-scale-um",
            "1",
            "--jitter",
            "0.000001",
            "--chains",
            "2",
            "--tune",
            "1000",
            "--draws",
            "1000",
            "--target-accept",
            "0.95",
            "--seed",
            "16101",
            "--maximum-events",
            "30",
            "--maximum-quadrature-nodes",
            "4",
            "--maximum-total-draw-node-work",
            "40000",
            "--prediction-replicates",
            "8",
            "--prediction-seed",
            "16102",
            "--maximum-predictive-points",
            "10000",
            "--neighbor-radius-um",
            "1.1",
            "--maximum-neighbor-pairs",
            "4",
            "--material-standardized-shift",
            "0.75",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.arbitrary_window_lgcp_sensitivity"
    );
    let scenarios = result["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 5);
    assert_eq!(result["all_fits_complete"], true);
    assert_eq!(result["total_draw_node_work"], 40000);
    assert_eq!(result["material_standardized_shift"], 0.75);
    let baseline = &scenarios[0]["posterior"];
    let mut requests = HashSet::new();
    let mut material = Vec::new();
    let mut overall_maximum = 0.0_f64;
    for (scenario, (name, amplitude, length_scale)) in scenarios.iter().zip([
        ("baseline", 0.2, 1.0),
        ("field_amplitude_lower", 0.1, 1.0),
        ("field_amplitude_upper", 0.4, 1.0),
        ("field_length_scale_lower", 0.2, 0.5),
        ("field_length_scale_upper", 0.2, 2.0),
    ]) {
        assert_eq!(scenario["name"], name);
        assert_eq!(scenario["field_amplitude"], amplitude);
        assert_eq!(scenario["field_length_scale_um"], length_scale);
        assert_eq!(scenario["backend"]["name"], "pymc");
        assert_eq!(scenario["source_backend"]["name"], "pymc");
        assert_eq!(scenario["fit_state"], "complete");
        assert_eq!(
            scenario["spatial_posterior_predictive"]["replicate_count"],
            8
        );
        let request = scenario["request_sha256"].as_str().unwrap();
        assert_eq!(request.len(), 64);
        assert!(
            requests.insert(request),
            "{name} reused an earlier fit request"
        );
        let mut maximum = scenario["latent_field_rms_standardized_shift"]
            .as_f64()
            .unwrap();
        assert!(maximum.is_finite() && maximum >= 0.0);
        for (parameter, field) in [
            ("intercept", "intercept_standardized_shift"),
            ("coefficient", "coefficient_standardized_shift"),
        ] {
            let scale = baseline[parameter]["sd"].as_f64().unwrap();
            assert!(scale > 0.0);
            let shift = (scenario["posterior"][parameter]["mean"].as_f64().unwrap()
                - baseline[parameter]["mean"].as_f64().unwrap())
            .abs()
                / scale;
            assert!(
                (scenario[field].as_f64().unwrap() - shift).abs() < 1e-12,
                "{name}, {parameter}"
            );
            maximum = maximum.max(shift);
        }
        assert!((scenario["maximum_standardized_shift"].as_f64().unwrap() - maximum).abs() < 1e-12);
        if name == "baseline" {
            assert_eq!(maximum, 0.0);
        }
        let changed = name != "baseline" && maximum >= 0.75;
        assert_eq!(scenario["material_change"], changed);
        if changed {
            material.push(name);
        }
        overall_maximum = overall_maximum.max(maximum);
    }
    assert!(
        (result["maximum_standardized_shift"].as_f64().unwrap() - overall_maximum).abs() < 1e-12
    );
    assert_eq!(result["material_scenarios"], serde_json::json!(material));
    assert_eq!(
        result["claim_status"],
        "experimental_fixed_kernel_sensitivity"
    );
}
