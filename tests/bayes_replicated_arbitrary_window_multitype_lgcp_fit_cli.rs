#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs, path::Path};

use assert_cmd::Command;

#[test]
fn replicated_multitype_lgcp_recovers_a_type_specific_group_intensity_shift() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("patterns.csv");
    let output = directory.path().join("fit.json");
    fs::write(&input, input_csv()).unwrap();

    fit_command(&input, &output, 192).assert().success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.bayesian_replicated_arbitrary_window_multitype_lgcp_fit"
    );
    assert_eq!(result["fit_state"], "complete", "{result}");
    assert_eq!(result["patient_count"], 8);
    assert_eq!(result["pattern_count"], 16);
    assert_eq!(result["type_count"], 3);
    assert_eq!(result["total_node_count"], 64);
    assert_eq!(result["total_node_type_row_count"], 192);
    assert_eq!(result["statistical_unit"], "patient");
    assert_eq!(
        result["likelihood"],
        "independent_type_specific_poisson_intensities_over_one_shared_exact_window"
    );
    let types = result["type_posteriors"].as_array().unwrap();
    let a = types.iter().find(|row| row["type_id"] == "A").unwrap();
    assert!(a["group_effect"]["interval_lower"].as_f64().unwrap() > 0.0);
    let pair = result["group_effect_differences"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["type_a"] == "A" && row["type_b"] == "B")
        .unwrap();
    assert!(pair["difference"]["interval_lower"].as_f64().unwrap() > 0.0);
    assert_eq!(result["diagnostics"]["divergences"], 0);
}

#[test]
fn replicated_multitype_lgcp_rejects_incomplete_types_and_one_short_row_work() {
    let directory = tempfile::tempdir().expect("tempdir");
    let incomplete = directory.path().join("incomplete.csv");
    let complete = directory.path().join("complete.csv");
    let output = directory.path().join("fit.json");
    let csv = input_csv();
    let mut removed = false;
    let incomplete_csv = csv
        .lines()
        .filter(|line| {
            if !removed && line.contains("q-0,B,") {
                removed = true;
                false
            } else {
                true
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&incomplete, format!("{incomplete_csv}\n")).unwrap();
    fs::write(&complete, csv).unwrap();

    fit_command(&incomplete, &output, 192)
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "requires every type exactly once",
        ));
    fit_command(&complete, &output, 191)
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "hierarchy or work ceiling differs",
        ));
}

fn input_csv() -> String {
    let mut csv = String::from(
        "pattern_id,patient_id,group,cohort,node_id,type_id,x_um,y_um,weight_um2,window_area_um2,covariate,count,window_sha256,event_sha256\n",
    );
    for group_index in 0..2 {
        let group = if group_index == 0 {
            "reference"
        } else {
            "comparison"
        };
        for patient_index in 0..4 {
            let patient = format!("{group}-p{patient_index}");
            for pattern_index in 0..2 {
                let pattern = format!("{patient}-s{pattern_index}");
                for node_index in 0..4 {
                    let ix = node_index % 2;
                    let iy = node_index / 2;
                    let covariate = ix * 2 - 1;
                    for (type_index, type_id) in ["A", "B", "C"].into_iter().enumerate() {
                        let base = match (group_index, type_index) {
                            (0, 0) => 3,
                            (1, 0) => 12,
                            (_, 1) => 6,
                            _ => 4,
                        };
                        let count = base + ix + pattern_index + patient_index % 2;
                        writeln!(
                            csv,
                            "{pattern},{patient},{group},synthetic,q-{node_index},{type_id},{},{},1,4,{covariate},{count},{:064x},{:064x}",
                            ix as f64 + 0.5,
                            iy as f64 + 0.5,
                            group_index * 100 + patient_index * 10 + pattern_index + 1,
                            group_index * 1000 + patient_index * 100 + pattern_index + 1,
                        )
                        .unwrap();
                    }
                }
            }
        }
    }
    csv
}

fn fit_command(input: &Path, output: &Path, maximum_rows: usize) -> Command {
    let mut command = Command::cargo_bin("marklab").expect("binary");
    let maximum_rows = maximum_rows.to_string();
    command.args([
        "bayes",
        "fit-replicated-arbitrary-window-multitype-lgcp",
        "--input",
        input.to_str().unwrap(),
        "--reference-group",
        "reference",
        "--comparison-group",
        "comparison",
        "--reference-type",
        "A",
        "--intercept-prior-mean",
        "1",
        "--intercept-prior-sd",
        "1",
        "--group-effect-prior-sd",
        "1",
        "--covariate-effect-prior-sd",
        "1",
        "--patient-sd-prior-scale",
        "0.5",
        "--pattern-sd-prior-scale",
        "0.5",
        "--field-amplitude",
        "0.05",
        "--field-length-scale-um",
        "1",
        "--jitter",
        "0.000001",
        "--chains",
        "2",
        "--tune",
        "1500",
        "--draws",
        "1500",
        "--target-accept",
        "0.95",
        "--seed",
        "20260829",
        "--maximum-patients",
        "8",
        "--maximum-patterns",
        "16",
        "--maximum-types",
        "3",
        "--maximum-nodes-per-pattern",
        "4",
        "--maximum-total-nodes",
        "64",
        "--maximum-total-node-type-rows",
        &maximum_rows,
        "--maximum-total-events",
        "5000",
        "--maximum-draw-node-type-work",
        "576000",
        "--timeout-seconds",
        "300",
        "--out",
        output.to_str().unwrap(),
    ]);
    command
}
