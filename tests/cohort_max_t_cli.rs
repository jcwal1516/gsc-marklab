#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn max_t_cli_matches_observed_hand_oracle() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("endpoints.csv");
    fs::write(
        &input,
        "patient_id,group,endpoint,value\n\
a-1,A,e1,8\n\
a-1,A,e2,4\n\
a-2,A,e1,9\n\
a-2,A,e2,7\n\
b-1,B,e1,1\n\
b-1,B,e2,2\n\
b-2,B,e1,2\n\
b-2,B,e2,3\n",
    )
    .expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "max-t",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--permutations",
            "99",
            "--seed",
            "41",
            "--alpha",
            "0.05",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_max_t");
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["randomization_unit"], "patient");
    assert!(result["design"].get("blocked").is_none());
    assert!(result["design"].get("block_count").is_none());
    assert_eq!(result["endpoints"][0]["endpoint"], "e1");
    assert_eq!(result["endpoints"][0]["effect_group_a_minus_group_b"], 7.0);
    assert!(
        (result["endpoints"][0]["studentized_statistic"]
            .as_f64()
            .expect("e1 statistic")
            - 9.899_494_936_611_665)
            .abs()
            < 1e-12
    );
    assert_eq!(result["endpoints"][1]["endpoint"], "e2");
    assert_eq!(result["endpoints"][1]["effect_group_a_minus_group_b"], 3.0);
    assert!(
        (result["endpoints"][1]["studentized_statistic"]
            .as_f64()
            .expect("e2 statistic")
            - 1.897_366_596_101_027_5)
            .abs()
            < 1e-12
    );
    assert_eq!(result["permutations"]["completed"], 99);
}

#[test]
fn max_t_cli_accepts_complete_patient_blocks_for_the_endpoint_family() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("blocked-endpoints.csv");
    let mut rows = String::from("patient_id,group,endpoint,value,block\n");
    for (patient, group, e1, e2, block) in [
        ("a-1", "A", 8, 4, "north"),
        ("a-2", "A", 9, 7, "north"),
        ("b-1", "B", 1, 2, "north"),
        ("b-2", "B", 2, 3, "north"),
        ("a-3", "A", 7, 5, "south"),
        ("a-4", "A", 10, 8, "south"),
        ("b-3", "B", 2, 1, "south"),
        ("b-4", "B", 3, 4, "south"),
    ] {
        rows.push_str(&format!("{patient},{group},e1,{e1},{block}\n"));
        rows.push_str(&format!("{patient},{group},e2,{e2},{block}\n"));
    }
    fs::write(&input, rows).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "max-t",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--permutations",
            "99",
            "--seed",
            "41",
            "--alpha",
            "0.05",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_max_t");
    assert_eq!(result["design"]["randomization_unit"], "patient");
    assert_eq!(result["design"]["null_family"], "population_independence");
    assert_eq!(result["design"]["blocked"], true);
    assert_eq!(result["design"]["block_count"], 2);
    assert_eq!(result["groups"]["group_a_patients"], 4);
    assert_eq!(result["groups"]["group_b_patients"], 4);
    assert_eq!(result["permutations"]["completed"], 99);
}

#[test]
fn max_t_cli_exposes_step_down_adjustment_for_the_complete_endpoint_family() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("endpoints.csv");
    fs::write(
        &input,
        "patient_id,group,endpoint,value\n\
a-1,A,strong,8\n\
a-1,A,middle,4\n\
a-1,A,weak,2\n\
a-2,A,strong,9\n\
a-2,A,middle,7\n\
a-2,A,weak,5\n\
b-1,B,strong,1\n\
b-1,B,middle,2\n\
b-1,B,weak,3\n\
b-2,B,strong,2\n\
b-2,B,middle,3\n\
b-2,B,weak,4\n",
    )
    .expect("fixture");
    let single_step_output = directory.path().join("single-step.json");
    let step_down_output = directory.path().join("step-down.json");

    for (output, step_down) in [(&single_step_output, false), (&step_down_output, true)] {
        let mut command = Command::cargo_bin("marklab").expect("binary");
        command.args([
            "cohort",
            "max-t",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--permutations",
            "199",
            "--seed",
            "41",
            "--alpha",
            "0.05",
        ]);
        if step_down {
            command.arg("--step-down");
        }
        command
            .args(["--out", output.to_str().expect("output path")])
            .assert()
            .success();
    }

    let single_step: serde_json::Value =
        serde_json::from_slice(&fs::read(single_step_output).expect("single-step result"))
            .expect("single-step JSON");
    let step_down: serde_json::Value =
        serde_json::from_slice(&fs::read(step_down_output).expect("step-down result"))
            .expect("step-down JSON");

    assert_eq!(step_down["design"]["correction"], "step_down_max_t");
    assert_eq!(step_down["design"]["randomization_unit"], "patient");
    assert_eq!(
        step_down["endpoints"]
            .as_array()
            .expect("step-down array")
            .len(),
        single_step["endpoints"]
            .as_array()
            .expect("single-step array")
            .len()
    );
    for index in 0..3 {
        assert_eq!(
            step_down["endpoints"][index]["endpoint"],
            single_step["endpoints"][index]["endpoint"]
        );
        assert!(
            step_down["endpoints"][index]["adjusted_p_value"]
                .as_f64()
                .expect("step-down p-value")
                <= single_step["endpoints"][index]["adjusted_p_value"]
                    .as_f64()
                    .expect("single-step p-value")
        );
    }
    let mut ordered = step_down["endpoints"]
        .as_array()
        .expect("endpoint array")
        .iter()
        .map(|endpoint| {
            (
                endpoint["studentized_statistic"]
                    .as_f64()
                    .expect("statistic")
                    .abs(),
                endpoint["adjusted_p_value"]
                    .as_f64()
                    .expect("step-down p-value"),
            )
        })
        .collect::<Vec<_>>();
    ordered.sort_by(|left, right| right.0.total_cmp(&left.0));
    assert!(ordered.windows(2).all(|pair| pair[0].1 <= pair[1].1));
}
