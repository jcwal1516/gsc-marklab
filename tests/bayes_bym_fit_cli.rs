#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn bym_fit_recovers_positive_effect_and_separate_ring_fields() {
    let directory = tempfile::tempdir().expect("tempdir");
    let regions = directory.path().join("regions.csv");
    let mut region_csv = String::from("region_id\n");
    for index in 0..12 {
        region_csv.push_str(&format!("r{index:02}\n"));
    }
    fs::write(&regions, region_csv).expect("regions");
    let edges = directory.path().join("edges.csv");
    let mut edge_csv = String::from("source_region,target_region,weight\n");
    for index in 0..12 {
        edge_csv.push_str(&format!("r{index:02},r{:02},1\n", (index + 11) % 12));
        edge_csv.push_str(&format!("r{index:02},r{:02},1\n", (index + 1) % 12));
    }
    fs::write(&edges, edge_csv).expect("edges");
    let data = directory.path().join("data.csv");
    fs::write(
        &data,
        "region_id,count,expected,x\n\
r00,93,100,-1\n\
r01,93,100,-0.8181818181818181\n\
r02,96,100,-0.6363636363636364\n\
r03,91,100,-0.4545454545454546\n\
r04,91,100,-0.2727272727272727\n\
r05,87,100,-0.09090909090909083\n\
r06,95,100,0.09090909090909083\n\
r07,102,100,0.2727272727272727\n\
r08,124,100,0.4545454545454546\n\
r09,141,100,0.6363636363636365\n\
r10,169,100,0.8181818181818183\n\
r11,194,100,1\n",
    )
    .expect("data");
    let output = directory.path().join("fit.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "bym-fit",
            "--regions",
            regions.to_str().unwrap(),
            "--edges",
            edges.to_str().unwrap(),
            "--data",
            data.to_str().unwrap(),
            "--intercept-prior-mean",
            "0",
            "--intercept-prior-sd",
            "2",
            "--coefficient-prior-sd",
            "1",
            "--structured-sd-prior",
            "1",
            "--unstructured-sd-prior",
            "1",
            "--chains",
            "2",
            "--tune",
            "2000",
            "--draws",
            "1000",
            "--target-accept",
            "0.97",
            "--seed",
            "8101",
            "--timeout-seconds",
            "180",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("BYM fit JSON");
    assert_eq!(result["format"], "marklab.bayesian_bym_fit");
    assert_eq!(result["fit_state"], "complete");
    assert_eq!(result["posterior"]["coefficients"][0]["predictor"], "x");
    assert!(
        result["posterior"]["coefficients"][0]["mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(result["rank_deficiency"], 1);
    assert_eq!(
        result["constraints"],
        serde_json::json!(["sum_to_zero:r00,r01,r02,r03,r04,r05,r06,r07,r08,r09,r10,r11"])
    );
    assert_eq!(result["regions"].as_array().unwrap().len(), 12);
    assert!(result["regions"].as_array().unwrap().iter().all(|region| {
        region["relative_risk"]["mean"]
            .as_f64()
            .is_some_and(f64::is_finite)
    }));
    assert_eq!(result["diagnostics"]["divergences"], 0);
    assert_eq!(result["diagnostics"]["max_tree_depth_hits"], 0);
    assert_eq!(result["claim_status"], "experimental");
}
