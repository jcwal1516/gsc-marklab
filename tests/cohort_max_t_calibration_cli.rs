#![cfg(feature = "cli")]

use std::{fs, process::Command};

use tempfile::tempdir;

#[test]
fn exhaustive_max_t_calibration_controls_the_complete_global_null() {
    let directory = tempdir().expect("temporary directory");
    let input = directory.path().join("patients.csv");
    let output = directory.path().join("calibration.json");
    fs::write(
        &input,
        concat!(
            "patient_id,e1,e2,e3\n",
            "p1,0,0,0\n",
            "p2,1,2,4\n",
            "p3,2,1,3\n",
            "p4,3,4,1\n",
            "p5,4,3,7\n",
            "p6,5,7,2\n",
            "p7,6,5,8\n",
            "p8,7,6,5\n",
        ),
    )
    .expect("input");

    let status = Command::new(env!("CARGO_BIN_EXE_marklab"))
        .args([
            "cohort",
            "max-t-calibration",
            "--input",
            input.to_str().expect("input path"),
            "--group-a-count",
            "4",
            "--family-sizes",
            "1,2",
            "--alpha",
            "0.1",
            "--maximum-assignments",
            "70",
            "--maximum-assignment-endpoint-evaluations",
            "63910",
            "--memory-budget-mib",
            "8",
            "--out",
            output.to_str().expect("output path"),
        ])
        .status()
        .expect("run max-t calibration");
    assert!(status.success());

    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(output).expect("result")).expect("result JSON");
    assert_eq!(result["format"], "marklab.cohort_max_t_calibration");
    assert_eq!(result["version"], 1);
    assert_eq!(
        result["null"],
        "every_fixed_size_whole_patient_group_assignment_is_equally_likely"
    );
    assert_eq!(result["patient_count"], 8);
    assert_eq!(result["exact_assignment_count"], 70);
    assert_eq!(result["family_sizes"], serde_json::json!([1, 2]));
    for method in ["single_step", "step_down", "ordered_gatekeeping_step_down"] {
        assert!(
            result[method]["familywise_error_rate"]
                .as_f64()
                .expect("FWER")
                <= 0.1
        );
        assert_eq!(
            result[method]["calibration_status"],
            "controlled_at_declared_alpha"
        );
    }
    assert_eq!(result["work"]["assignment_endpoint_comparisons"], 63910);
}
