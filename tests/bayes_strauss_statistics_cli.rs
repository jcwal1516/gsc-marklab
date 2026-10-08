#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn strauss_pair_statistic_and_papangelou_match_hand_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("points.csv");
    let output = directory.path().join("strauss.json");
    fs::write(&input, "point_id,x_um,y_um\na,0,0\nb,3,4\nc,10,0\n").expect("points");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "strauss-statistics",
            "--input",
            input.to_str().unwrap(),
            "--interaction-radius-um",
            "5",
            "--proposal-x-um",
            "4",
            "--proposal-y-um",
            "0",
            "--beta-per-um2",
            "2",
            "--gamma",
            "0.5",
            "--maximum-pair-visits",
            "100",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).unwrap()).expect("Strauss JSON");
    assert_eq!(result["format"], "marklab.strauss_statistics");
    assert_eq!(
        result["pair_boundary"],
        "euclidean_distance_less_than_or_equal_radius"
    );
    assert_eq!(result["point_count"], 3);
    assert_eq!(result["unordered_pair_visits"], 3);
    assert_eq!(result["interacting_pair_count"], 1);
    assert_eq!(result["proposal"]["neighbor_delta"], 2);
    assert!((result["proposal"]["papangelou_per_um2"].as_f64().unwrap() - 0.5).abs() <= 1e-12);
    assert_eq!(
        result["claim_status"],
        "experimental_fixed_pattern_mechanics"
    );
}
