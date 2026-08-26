#![cfg(feature = "cli")]

use assert_cmd::Command;
use std::fs;

#[test]
fn embedding_spatial_envelope_is_complete_vector_erl_and_seed_deterministic() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("embeddings.csv");
    let bins = directory.path().join("bins.csv");
    let output = directory.path().join("output.json");
    let repeated = directory.path().join("repeated.json");
    fs::write(
        &input,
        "object_id,permutation_stratum,x_um,y_um,embedding_0,embedding_1\na0,s1,0,0,0,0\na1,s1,1,0,0.1,0.1\na2,s1,3,0,3,3\na3,s1,6,0,3.1,3.1\nb0,s2,10,0,0,0\nb1,s2,11,0,0.1,0.1\nb2,s2,13,0,3,3\nb3,s2,16,0,3.1,3.1\n",
    )
    .unwrap();
    fs::write(
        &bins,
        "bin_id,lower_um,upper_um\nnear,0,2\nmid,2,5\nfar,5,17\n",
    )
    .unwrap();

    for out in [&output, &repeated] {
        Command::cargo_bin("marklab")
            .unwrap()
            .args([
                "bayes",
                "embedding-spatial-dependence-envelope",
                "--input",
                input.to_str().unwrap(),
                "--bins",
                bins.to_str().unwrap(),
                "--curve",
                "vector_semivariogram",
                "--permutations",
                "39",
                "--alpha",
                "0.05",
                "--seed",
                "991",
                "--maximum-pair-visits",
                "1120",
                "--out",
                out.to_str().unwrap(),
            ])
            .assert()
            .success();
    }
    assert_eq!(fs::read(&output).unwrap(), fs::read(&repeated).unwrap());

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.embedding_spatial_dependence_envelope"
    );
    assert_eq!(result["version"], 1);
    assert_eq!(result["curve_function"], "vector_semivariogram");
    assert_eq!(
        result["permutation_policy"],
        "complete_embedding_rows_within_declared_strata"
    );
    assert_eq!(result["permutations"], 39);
    assert_eq!(result["pair_visits"], 1120);
    assert_eq!(result["curve"][0]["pair_count"], 2);
    assert_eq!(result["curve"][1]["pair_count"], 7);
    assert_eq!(result["curve"][2]["pair_count"], 19);
    let p = result["p_global"].as_f64().unwrap();
    assert!((0.025..=1.0).contains(&p));
    assert!((p * 40.0 - (p * 40.0).round()).abs() < 1e-12);
    assert!((0.0..=1.0).contains(&result["erl_depth"].as_f64().unwrap()));
    assert!((0.0..=1.0).contains(&result["critical_depth"].as_f64().unwrap()));
    for row in result["curve"].as_array().unwrap() {
        assert!(row["observed"].as_f64().unwrap().is_finite());
        assert!(row["lower"].as_f64().unwrap().is_finite());
        assert!(row["upper"].as_f64().unwrap().is_finite());
    }
}
