#![cfg(feature = "cli")]

use std::fs;

use assert_cmd::Command;

#[test]
fn hierarchical_max_t_cli_opens_only_ordered_complete_families() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("hierarchical_max_t.csv");
    let mut csv = String::from("patient_id,group,family_order,family,endpoint,value\n");
    let rows = [
        ("a1", "A", [8.0, 7.0, 5.0]),
        ("a2", "A", [9.0, 8.0, 6.0]),
        ("a3", "A", [10.0, 9.0, 7.0]),
        ("a4", "A", [11.0, 10.0, 8.0]),
        ("b1", "B", [1.0, 1.5, 4.0]),
        ("b2", "B", [2.0, 2.5, 4.5]),
        ("b3", "B", [3.0, 3.5, 5.0]),
        ("b4", "B", [4.0, 4.5, 5.5]),
    ];
    for (patient, group, values) in rows {
        csv.push_str(&format!(
            "{patient},{group},1,primary,response,{value}\n",
            value = values[0]
        ));
        csv.push_str(&format!(
            "{patient},{group},1,primary,stability,{value}\n",
            value = values[1]
        ));
        csv.push_str(&format!(
            "{patient},{group},2,secondary,mechanism,{value}\n",
            value = values[2]
        ));
    }
    fs::write(&input, csv).expect("fixture");
    let output = directory.path().join("result.json");

    Command::cargo_bin("marklab")
        .expect("binary")
        .args([
            "cohort",
            "hierarchical-max-t",
            "--input",
            input.to_str().expect("input path"),
            "--group-a",
            "A",
            "--group-b",
            "B",
            "--permutations",
            "199",
            "--seed",
            "43",
            "--alpha",
            "0.2",
            "--step-down",
            "--out",
            output.to_str().expect("output path"),
        ])
        .assert()
        .success();

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("JSON");
    assert_eq!(result["format"], "marklab.cohort_hierarchical_max_t");
    assert_eq!(result["version"], 1);
    assert_eq!(result["design"]["analysis_level"], "patient");
    assert_eq!(result["design"]["null_family"], "population_independence");
    assert_eq!(result["design"]["permutation_unit"], "patient_label");
    assert_eq!(
        result["design"]["multiplicity"],
        "ordered_family_gatekeeping_max_t"
    );
    assert_eq!(result["design"]["correction"], "step_down_max_t");
    assert_eq!(result["families"].as_array().expect("families").len(), 2);
    assert_eq!(result["families"][0]["family"], "primary");
    assert_eq!(result["families"][0]["opened"], true);
    assert_eq!(result["families"][0]["all_endpoints_rejected"], true);
    assert_eq!(result["families"][1]["family"], "secondary");
    assert_eq!(result["families"][1]["opened"], true);
    assert_eq!(result["patients"]["total"], 8);
    assert_eq!(result["patients"]["group_a"], 4);
    assert_eq!(result["patients"]["group_b"], 4);
    assert_eq!(result["permutations"]["completed"], 199);
}
