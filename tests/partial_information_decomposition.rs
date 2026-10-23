use serde_json::{json, Value};

fn request(counts: &[[u64; 4]]) -> Value {
    json!({
        "format": "marklab.partial_information_decomposition_request", "version": 1,
        "estimator": "williams_beer_i_min", "statistical_unit": "patient",
        "target": {"name": "response", "levels": ["0", "1"], "definition": "declared binary response"},
        "sources": [
            {"name": "morphology", "levels": ["0", "1"], "definition": "prespecified feature bins"},
            {"name": "cells", "levels": ["0", "1"], "definition": "prespecified feature bins"}
        ],
        "counts": counts.iter().map(|r| json!({"target": r[0], "source_a": r[1], "source_b": r[2], "count": r[3]})).collect::<Vec<_>>(),
        "limits": {"maximum_entries": 100, "maximum_states": 100, "maximum_observations": 10000}
    })
}

fn analyze(input: &Value) -> Value {
    serde_json::to_value(
        marklab::marks::partial_information_decomposition_from_json(
            &serde_json::to_vec(input).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn known_copy_xor_unique_and_independent_information_in_bits() {
    let independent: Vec<_> = (0..2)
        .flat_map(|t| (0..2).flat_map(move |a| (0..2).map(move |b| [t, a, b, 1])))
        .collect();
    let cases: &[(&[[u64; 4]], [f64; 4])] = &[
        (&[[0, 0, 0, 1], [1, 1, 1, 1]], [1.0, 0.0, 0.0, 0.0]),
        (
            &[[0, 0, 0, 1], [1, 0, 1, 1], [1, 1, 0, 1], [0, 1, 1, 1]],
            [0.0, 0.0, 0.0, 1.0],
        ),
        (
            &[[0, 0, 0, 1], [0, 0, 1, 1], [1, 1, 0, 1], [1, 1, 1, 1]],
            [0.0, 1.0, 0.0, 0.0],
        ),
        (&independent, [0.0; 4]),
    ];
    for (counts, expected) in cases {
        let out = analyze(&request(counts));
        let actual = [
            "redundancy_bits",
            "unique_source_a_bits",
            "unique_source_b_bits",
            "synergy_bits",
        ]
        .map(|key| out[key].as_f64().unwrap());
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }
        assert!(
            (actual.iter().sum::<f64>() - out["joint_information_bits"].as_f64().unwrap()).abs()
                < 1e-12
        );
        assert_eq!(out["estimator"], "williams_beer_i_min");
    }
    // I_min compares information amounts: two independent bits in a joint target
    // still produce one redundant and one synergistic bit under this definition.
    let mut joint_target = request(&[[0, 0, 0, 1], [1, 0, 1, 1], [2, 1, 0, 1], [3, 1, 1, 1]]);
    joint_target["target"]["levels"] = json!(["00", "01", "10", "11"]);
    let out = analyze(&joint_target);
    assert_eq!(out["joint_information_bits"], 2.0);
    assert_eq!(out["redundancy_bits"], 1.0);
    assert_eq!(out["synergy_bits"], 1.0);
}

#[test]
fn counts_are_canonical_and_information_is_scale_and_source_order_invariant() {
    let counts = [[0, 0, 0, 3], [0, 0, 1, 2], [1, 1, 0, 4], [1, 1, 1, 1]];
    let original = analyze(&request(&counts));
    let mut reversed = counts;
    reversed.reverse();
    assert_eq!(original, analyze(&request(&reversed)));
    let split = [
        [0, 0, 0, 1],
        [0, 0, 0, 2],
        [0, 0, 1, 2],
        [1, 1, 0, 4],
        [1, 1, 1, 1],
    ];
    assert_eq!(original, analyze(&request(&split)));
    let scaled = analyze(&request(&counts.map(|[t, a, b, n]| [t, a, b, n * 10])));
    for key in [
        "target_entropy_bits",
        "joint_information_bits",
        "redundancy_bits",
        "unique_source_a_bits",
        "unique_source_b_bits",
        "synergy_bits",
    ] {
        assert!((original[key].as_f64().unwrap() - scaled[key].as_f64().unwrap()).abs() < 1e-12);
    }
    let swapped = analyze(&request(&counts.map(|[t, a, b, n]| [t, b, a, n])));
    assert!(
        (original["unique_source_a_bits"].as_f64().unwrap()
            - swapped["unique_source_b_bits"].as_f64().unwrap())
        .abs()
            < 1e-12
    );
    let constant = analyze(&request(&[[0, 0, 0, 1], [0, 1, 1, 1]]));
    assert_eq!(constant["target_entropy_bits"], 0.0);
    assert_eq!(constant["joint_information_bits"], 0.0);
}

#[test]
fn malformed_counts_and_unadmitted_state_spaces_fail() {
    let good = request(&[[0, 0, 0, 3], [1, 1, 1, 1]]);
    let mut invalid = Vec::new();
    let mut v = good.clone();
    v["counts"][0]["target"] = json!(2);
    invalid.push(v);
    let mut v = good.clone();
    v["counts"][0]["count"] = json!(0);
    invalid.push(v);
    let mut v = good.clone();
    v["counts"][0]["count"] = json!(u64::MAX);
    invalid.push(v);
    let mut v = good.clone();
    v["limits"]["maximum_states"] = json!(7);
    invalid.push(v);
    let mut v = good.clone();
    v["limits"]["maximum_observations"] = json!(3);
    invalid.push(v);
    let mut v = good.clone();
    v["sources"][1]["name"] = v["sources"][0]["name"].clone();
    invalid.push(v);
    let mut v = good;
    v["estimator"] = json!("unspecified");
    invalid.push(v);
    for input in invalid {
        assert!(marklab::marks::partial_information_decomposition_from_json(
            &serde_json::to_vec(&input).unwrap()
        )
        .is_err());
    }
}

#[cfg(feature = "cli")]
#[test]
fn pid_cli_publishes_the_real_result_without_overwriting() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    let output = directory.path().join("pid.json");
    std::fs::write(
        &input,
        serde_json::to_vec(&request(&[[0, 0, 0, 1], [1, 1, 1, 1]])).unwrap(),
    )
    .unwrap();
    let invoke = || {
        let mut command = assert_cmd::Command::cargo_bin("marklab").unwrap();
        command
            .args(["numerics", "partial-information-decomposition", "--input"])
            .arg(&input)
            .arg("--out")
            .arg(&output);
        command
    };
    invoke().assert().success();
    let bytes = std::fs::read(&output).unwrap();
    let result: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(result["redundancy_bits"], 1.0);
    invoke().assert().failure();
    assert_eq!(std::fs::read(output).unwrap(), bytes);
}
