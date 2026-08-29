#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn replicated_conditional_marks_run_the_fixed_hierarchy_prior_grid() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.csv");
    let pymc = directory.path().join("pymc.json");
    let sensitivity = directory.path().join("sensitivity.json");
    let mut csv = String::from("pattern_id,patient_id,group,point_id,x_um,y_um,type_id\n");
    for (group_index, group) in ["MSS", "MSI"].into_iter().enumerate() {
        for patient_index in 0..4 {
            let patient = format!("{group}-p{patient_index}");
            for slide_index in 0..2 {
                let pattern = format!("{patient}-s{slide_index}");
                for point_index in 0..45 {
                    let type_index = point_index % 3;
                    let type_id = ["A", "B", "C"][type_index];
                    let (x, y) = if group_index == 0 {
                        (
                            type_index as f64 * 100.0 + (point_index / 3 % 5) as f64,
                            (point_index / 15) as f64,
                        )
                    } else {
                        (
                            (point_index / 3 % 5) as f64 * 3.0 + type_index as f64,
                            (point_index / 15) as f64,
                        )
                    };
                    writeln!(
                        csv,
                        "{pattern},{patient},{group},{pattern}-{point_index:03},{x},{y},{type_id}"
                    )
                    .unwrap();
                }
            }
        }
    }
    fs::write(&input, csv).unwrap();

    let common = [
        "--input",
        input.to_str().unwrap(),
        "--reference-group",
        "MSS",
        "--reference-type",
        "A",
        "--radius-um",
        "1.5",
        "--intercept-prior-sd",
        "2",
        "--interaction-prior-sd",
        "1",
        "--group-effect-prior-sd",
        "1",
        "--patient-sd-prior-scale",
        "0.5",
        "--pattern-sd-prior-scale",
        "0.5",
        "--chains",
        "2",
        "--tune",
        "1000",
        "--draws",
        "1500",
        "--target-accept",
        "0.95",
        "--seed",
        "20260829",
        "--maximum-patients",
        "10",
        "--maximum-patterns",
        "20",
        "--maximum-points",
        "1000",
        "--maximum-types",
        "4",
        "--maximum-neighbor-visits",
        "100000",
        "--maximum-edges",
        "10000",
        "--maximum-draw-parameter-work",
        "1000000",
        "--maximum-working-bytes",
        "16777216",
    ];
    Command::cargo_bin("marklab")
        .expect("binary")
        .args(["bayes", "fit-replicated-conditional-multitype-mark"])
        .args(common)
        .args([
            "--maximum-tree-depth",
            "12",
            "--timeout-seconds",
            "300",
            "--out",
            pymc.to_str().unwrap(),
        ])
        .assert()
        .success();

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "bayes",
            "replicated-conditional-multitype-mark-sensitivity",
            "--pymc-result",
            pymc.to_str().unwrap(),
        ])
        .args(common)
        .args([
            "--pymc-maximum-tree-depth",
            "12",
            "--numpyro-maximum-tree-depth",
            "12",
            "--maximum-processes",
            "4",
            "--maximum-total-draw-parameter-work",
            "3000000",
            "--materiality-standard-deviations",
            "0.75",
            "--timeout-seconds",
            "300",
            "--out",
            sensitivity.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(sensitivity).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.replicated_conditional_multitype_mark_prior_sensitivity"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    let scenarios = result["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 4);
    assert_eq!(
        scenarios
            .iter()
            .map(|row| row["scenario"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "patient_scale_half",
            "patient_scale_double",
            "pattern_scale_half",
            "pattern_scale_double",
        ]
    );
    for scenario in scenarios {
        assert_eq!(scenario["fit"]["fit_state"], "complete", "{scenario}");
        assert!(
            scenario["fit"]["group_affinity_shifts"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["contrast"]["interval_lower"].as_f64().unwrap() > 0.0),
            "{scenario}"
        );
    }
    assert_eq!(result["statistical_unit"], "patient");
}
