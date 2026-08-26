#![cfg(feature = "cli")]

use assert_cmd::Command;
use std::fs;

#[test]
fn geyer_pointwise_saturation_matches_hand_oracle() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("points.csv");
    let output = directory.path().join("geyer.json");
    fs::write(&input, "point_id,x_um,y_um\na,0,0\nb,3,4\nc,10,0\n").unwrap();
    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "geyer-saturation-statistic",
            "--input",
            input.to_str().unwrap(),
            "--interaction-radius-um",
            "5",
            "--saturation",
            "1",
            "--maximum-pair-visits",
            "100",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(result["format"], "marklab.geyer_saturation_statistic");
    assert_eq!(result["statistic"], 2);
    assert_eq!(result["unordered_pair_visits"], 3);
    let points = result["points"].as_array().unwrap();
    assert_eq!(points[0]["neighbor_count"], 1);
    assert_eq!(points[1]["neighbor_count"], 1);
    assert_eq!(points[2]["neighbor_count"], 0);
    assert!(points
        .iter()
        .all(|p| p["saturated_count"].as_u64().unwrap() <= 1));
    assert_eq!(
        result["convention"],
        "sum_over_points_of_min_saturation_and_inclusive_radius_neighbor_count"
    );
}
