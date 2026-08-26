#![cfg(feature = "cli")]

use assert_cmd::Command;
use std::fs;

#[test]
fn cross_modal_covariance_matches_hand_matrices_and_stratified_max_t() {
    let directory = tempfile::tempdir().unwrap();
    let a = directory.path().join("a.csv");
    let b = directory.path().join("b.csv");
    let pairs = directory.path().join("pairs.csv");
    let bins = directory.path().join("bins.csv");
    let output = directory.path().join("output.json");
    fs::write(
        &a,
        "object_id,source_section,compartment,embedding_0,embedding_1\na0,s1,tumor,0,0\na1,s1,tumor,2,0\na2,s1,tumor,0,2\na3,s1,tumor,2,2\n",
    )
    .unwrap();
    fs::write(
        &b,
        "object_id,source_section,compartment,embedding_0,embedding_1\nb0,s1,tumor,0,0\nb1,s1,tumor,2,0\nb2,s1,tumor,0,2\nb3,s1,tumor,2,2\n",
    )
    .unwrap();
    fs::write(
        &pairs,
        "a_object_id,b_object_id,source_section,compartment,bin_id,weight\na0,b0,s1,tumor,near,1\na1,b1,s1,tumor,near,1\na2,b3,s1,tumor,far,1\na3,b2,s1,tumor,far,1\n",
    )
    .unwrap();
    fs::write(&bins, "bin_id,lower_um,upper_um\nnear,0,10\nfar,10,20\n").unwrap();

    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "cross-modal-covariance-by-distance",
            "--a-input",
            a.to_str().unwrap(),
            "--b-input",
            b.to_str().unwrap(),
            "--pairs",
            pairs.to_str().unwrap(),
            "--bins",
            bins.to_str().unwrap(),
            "--mean-policy",
            "global",
            "--permutations",
            "20",
            "--seed",
            "811",
            "--maximum-pairs",
            "4",
            "--maximum-component-pair-visits",
            "336",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.cross_modal_covariance_by_distance"
    );
    assert_eq!(result["version"], 1);
    assert_eq!(result["mean_policy"], "global");
    assert_eq!(
        result["inference_policy"],
        "source_section_compartment_random_labeling"
    );
    assert_eq!(result["multiplicity_control"], "single_step_max_t");
    assert_eq!(result["pair_count"], 4);
    assert_eq!(result["permutations"], 20);
    let matrices = &result["matrix_artifact"]["matrices"];
    let expected = [[[1.0, 0.0], [0.0, 1.0]], [[-1.0, 0.0], [0.0, 1.0]]];
    for bin in 0..2 {
        assert_eq!(result["summaries"][bin]["pair_count"], 2);
        assert!(
            (result["summaries"][bin]["frobenius_norm"].as_f64().unwrap() - 2.0_f64.sqrt()).abs()
                < 1e-12
        );
        for row in 0..2 {
            for column in 0..2 {
                assert!(
                    (matrices[bin]["matrix"][row][column].as_f64().unwrap()
                        - expected[bin][row][column])
                        .abs()
                        < 1e-12
                );
            }
        }
        if let Some(adjusted_p) = result["summaries"][bin]["max_t_adjusted_p"].as_f64() {
            assert!((1.0 / 21.0..=1.0).contains(&adjusted_p));
        }
    }
}
