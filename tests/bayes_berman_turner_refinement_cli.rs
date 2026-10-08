#![cfg(feature = "cli")]

use std::{f64::consts::LN_2, fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn constant_intensity_coarse_and_fine_tables_match_exact_objective() {
    let directory = tempfile::tempdir().expect("tempdir");
    let events = directory.path().join("events.csv");
    let coarse = directory.path().join("coarse.csv");
    let fine = directory.path().join("fine.csv");
    let output = directory.path().join("berman-turner.json");
    fs::write(
        &events,
        "event_id,x_um,y_um,covariate,offset\na,0.25,0.25,0,0\nb,1.75,0.75,0,0\n",
    )
    .expect("events");
    fs::write(&coarse, "ix,iy,covariate,offset\n0,0,0,0\n1,0,0,0\n").expect("coarse");
    let mut fine_csv = String::from("ix,iy,covariate,offset\n");
    for iy in 0..2 {
        for ix in 0..4 {
            writeln!(fine_csv, "{ix},{iy},0,0").expect("fine row");
        }
    }
    fs::write(&fine, fine_csv).expect("fine");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "berman-turner-refinement",
            "--events",
            events.to_str().unwrap(),
            "--coarse-quadrature",
            coarse.to_str().unwrap(),
            "--fine-quadrature",
            fine.to_str().unwrap(),
            "--xmin-um",
            "0",
            "--ymin-um",
            "0",
            "--xmax-um",
            "2",
            "--ymax-um",
            "1",
            "--coarse-grid-x",
            "2",
            "--coarse-grid-y",
            "1",
            "--fine-grid-x",
            "4",
            "--fine-grid-y",
            "2",
            "--intercept",
            "0.6931471805599453",
            "--coefficient",
            "0",
            "--convergence-tolerance",
            "0.000000000001",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("Berman-Turner JSON");
    let exact = 2.0 * LN_2 - 4.0;
    assert_eq!(result["format"], "marklab.berman_turner_refinement");
    assert!((result["coarse"]["weight_sum_um2"].as_f64().unwrap() - 2.0).abs() <= 1e-12);
    assert!((result["fine"]["weight_sum_um2"].as_f64().unwrap() - 2.0).abs() <= 1e-12);
    assert!((result["coarse"]["weighted_objective"].as_f64().unwrap() - exact).abs() <= 1e-12);
    assert!((result["fine"]["weighted_objective"].as_f64().unwrap() - exact).abs() <= 1e-12);
    assert!(result["absolute_objective_change"].as_f64().unwrap() <= 1e-12);
    assert_eq!(result["converged"], true);
    assert_eq!(result["fine_table"].as_array().unwrap().len(), 10);
    assert_eq!(
        result["claim_status"],
        "experimental_approximation_diagnostic"
    );
}
