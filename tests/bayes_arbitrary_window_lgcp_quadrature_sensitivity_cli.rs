#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;

#[test]
fn exact_window_lgcp_compares_three_area_conserving_physical_partitions() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let window = directory.path().join("window.geojson");
    let output = directory.path().join("sensitivity.json");
    let mut events_csv = String::from("event_id,x_um,y_um,covariate,offset\n");
    let mut event_rows = Vec::new();
    for iy in 0..2 {
        for ix in 0..4 {
            for replicate in 0..2 {
                let id = format!("e-{iy}-{ix}-{replicate}");
                let x = ix as f64 + 0.25 + replicate as f64 * 0.5;
                let y = iy as f64 + 0.5;
                let covariate = if x < 2.0 { -1 } else { 1 };
                writeln!(events_csv, "{id},{x},{y},{covariate},0").unwrap();
                event_rows.push((id, x, y));
            }
        }
    }
    fs::write(&events, events_csv).unwrap();
    fs::write(
        &window,
        r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[4,0],[4,2],[0,2],[0,0]]]]}"#,
    )
    .unwrap();
    let coarse = write_partition(directory.path(), "coarse", 2, &event_rows);
    let intermediate = write_partition(directory.path(), "intermediate", 3, &event_rows);
    let baseline = write_partition(directory.path(), "baseline", 4, &event_rows);

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "arbitrary-window-lgcp-quadrature-sensitivity",
            "--events",
            events.to_str().unwrap(),
            "--window",
            window.to_str().unwrap(),
            "--coarse-event-membership",
            coarse.0.to_str().unwrap(),
            "--coarse-quadrature",
            coarse.1.to_str().unwrap(),
            "--intermediate-event-membership",
            intermediate.0.to_str().unwrap(),
            "--intermediate-quadrature",
            intermediate.1.to_str().unwrap(),
            "--baseline-event-membership",
            baseline.0.to_str().unwrap(),
            "--baseline-quadrature",
            baseline.1.to_str().unwrap(),
            "--intercept-prior-mean",
            "0.6931471805599453",
            "--intercept-prior-sd",
            "0.5",
            "--coefficient-prior-mean",
            "0",
            "--coefficient-prior-sd",
            "0.5",
            "--field-amplitude",
            "0.000001",
            "--field-length-scale-um",
            "1",
            "--jitter",
            "0.00000001",
            "--chains",
            "2",
            "--tune",
            "750",
            "--draws",
            "1000",
            "--target-accept",
            "0.95",
            "--seed",
            "16101",
            "--maximum-events",
            "16",
            "--maximum-quadrature-nodes",
            "8",
            "--maximum-total-draw-node-work",
            "36000",
            "--prediction-replicates",
            "8",
            "--prediction-seed",
            "16102",
            "--maximum-predictive-points",
            "10000",
            "--neighbor-radius-um",
            "1.1",
            "--maximum-neighbor-pairs",
            "20",
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
        "marklab.arbitrary_window_lgcp_quadrature_sensitivity"
    );
    assert_eq!(result["resolutions"].as_array().unwrap().len(), 3);
    assert_eq!(result["all_fits_complete"], true);
    assert_eq!(result["total_draw_node_work"], 36000);
    assert_eq!(result["resolutions"][0]["quadrature_node_count"], 4);
    assert_eq!(result["resolutions"][1]["quadrature_node_count"], 6);
    assert_eq!(result["resolutions"][2]["quadrature_node_count"], 8);
    assert!(result["maximum_standardized_shift"].as_f64().unwrap() < 0.75);
    assert_eq!(result["material_change"], false);
    assert_eq!(
        result["claim_status"],
        "experimental_quadrature_sensitivity"
    );
}

fn write_partition(
    directory: &Path,
    name: &str,
    grid_x: usize,
    events: &[(String, f64, f64)],
) -> (std::path::PathBuf, std::path::PathBuf) {
    let membership = directory.join(format!("{name}-membership.csv"));
    let quadrature = directory.join(format!("{name}-quadrature.csv"));
    let cell_width = 4.0 / grid_x as f64;
    let mut membership_csv = String::from("event_id,quadrature_node_id\n");
    for (event_id, x, y) in events {
        let ix = ((*x / cell_width).floor() as usize).min(grid_x - 1);
        let iy = (*y).floor() as usize;
        writeln!(membership_csv, "{event_id},q-{iy}-{ix}").unwrap();
    }
    let mut quadrature_csv = String::from("node_id,x_um,y_um,weight_um2,covariate,offset\n");
    for iy in 0..2 {
        for ix in 0..grid_x {
            let x = (ix as f64 + 0.5) * cell_width;
            let covariate = if x < 2.0 { -1 } else { 1 };
            writeln!(
                quadrature_csv,
                "q-{iy}-{ix},{x},{},{cell_width},{covariate},0",
                iy as f64 + 0.5
            )
            .unwrap();
        }
    }
    fs::write(&membership, membership_csv).unwrap();
    fs::write(&quadrature, quadrature_csv).unwrap();
    (membership, quadrature)
}
