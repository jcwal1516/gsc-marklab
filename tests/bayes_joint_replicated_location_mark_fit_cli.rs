#![cfg(feature = "cli")]

use std::{fmt::Write as _, fs};

use assert_cmd::Command;

#[test]
fn joint_location_mark_fit_uses_prespecified_pattern_holdout_and_patient_ecology() {
    let directory = tempfile::tempdir().unwrap();
    let location = directory.path().join("location.csv");
    let marks = directory.path().join("marks.csv");
    let output = directory.path().join("joint.json");
    let (location_csv, mark_csv) = fixtures();
    fs::write(&location, location_csv).unwrap();
    fs::write(&marks, mark_csv).unwrap();

    Command::cargo_bin("marklab")
        .unwrap()
        .args([
            "bayes",
            "fit-joint-replicated-location-mark",
            "--location-input",
            location.to_str().unwrap(),
            "--mark-input",
            marks.to_str().unwrap(),
            "--reference-group",
            "reference",
            "--comparison-group",
            "comparison",
            "--reference-type",
            "A",
            "--neighbor-radius-um",
            "1.1",
            "--location-prior-sd",
            "1",
            "--group-prior-sd",
            "1",
            "--patient-ecology-prior-scale",
            "0.5",
            "--mark-intercept-prior-sd",
            "1",
            "--mark-group-prior-sd",
            "1",
            "--mark-loading-prior-sd",
            "0.5",
            "--interaction-prior-sd",
            "0.5",
            "--chains",
            "2",
            "--tune",
            "100",
            "--draws",
            "100",
            "--target-accept",
            "0.9",
            "--seed",
            "20260831",
            "--maximum-patients",
            "8",
            "--maximum-patterns",
            "16",
            "--maximum-types",
            "3",
            "--maximum-location-rows",
            "192",
            "--maximum-mark-points",
            "1000",
            "--maximum-neighbor-visits",
            "100000",
            "--maximum-draw-observation-work",
            "1000000",
            "--maximum-working-bytes",
            "536870912",
            "--maximum-tree-depth",
            "10",
            "--timeout-seconds",
            "600",
            "--out",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let result: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(
        result["format"],
        "marklab.joint_replicated_location_mark_fit"
    );
    assert_eq!(result["statistical_unit"], "patient");
    assert_eq!(result["patient_count"], 4);
    assert_eq!(result["pattern_count"], 8);
    assert_eq!(result["training_pattern_count"], 4);
    assert_eq!(result["heldout_pattern_count"], 4);
    assert_eq!(result["type_ids"], serde_json::json!(["A", "B", "C"]));
    assert!(
        result["joint_posterior"]["patient_ecology_sd"]["mean"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    for name in ["joint", "location_only", "conditional_mark_only"] {
        assert!(
            result["heldout_comparison"][name]["mean_log_predictive_density"]
                .as_f64()
                .unwrap()
                .is_finite()
        );
    }
    assert!(
        result["heldout_comparison"]["joint_minus_separate_baselines"]["mean"]
            .as_f64()
            .unwrap()
            .is_finite()
    );
    for name in ["total_count", "mark_proportions", "cross_type_enrichment"] {
        assert!(
            result["posterior_predictive"][name]["two_sided_tail_probability"]
                .as_f64()
                .unwrap()
                .is_finite()
        );
    }
    if result["fit_state"] == "complete" {
        assert_eq!(
            result["claim_status"],
            "joint_spatial_association_not_communication_or_causality"
        );
    } else {
        assert_eq!(result["fit_state"], "nonconverged");
        assert_eq!(
            result["claim_status"],
            "diagnostic_only_nonconverged_joint_location_mark"
        );
    }
}

fn fixtures() -> (String, String) {
    let mut location = String::from(
        "pattern_id,patient_id,group,cohort,node_id,type_id,x_um,y_um,weight_um2,window_area_um2,covariate,count,window_sha256,event_sha256\n",
    );
    let mut marks = String::from("pattern_id,patient_id,group,point_id,x_um,y_um,type_id\n");
    for group_index in 0..2 {
        let group = if group_index == 0 {
            "reference"
        } else {
            "comparison"
        };
        for patient_index in 0..2 {
            let patient = format!("{group}-p{patient_index}");
            for pattern_index in 0..2 {
                let pattern = format!("{patient}-s{pattern_index}");
                for node_index in 0..4 {
                    let x = (node_index % 2) as f64 + 0.5;
                    let y = (node_index / 2) as f64 + 0.5;
                    for (type_index, type_id) in ["A", "B", "C"].into_iter().enumerate() {
                        let count =
                            2 + type_index + usize::from(group_index == 1 && type_id == "A");
                        writeln!(location,
                            "{pattern},{patient},{group},synthetic,q-{node_index},{type_id},{x},{y},1,4,{}, {count},{:064x},{:064x}",
                            node_index % 2 * 2 - 1,
                            group_index * 100 + patient_index * 10 + pattern_index + 1,
                            group_index * 1000 + patient_index * 100 + pattern_index + 1,
                        ).unwrap();
                    }
                }
                for point_index in 0..18 {
                    let type_id = ["A", "B", "C"][(point_index + group_index) % 3];
                    let x = (point_index % 6) as f64 * 0.3 + 0.1;
                    let y = (point_index / 6) as f64 * 0.6 + 0.1;
                    writeln!(
                        marks,
                        "{pattern},{patient},{group},{pattern}:{point_index:03},{x},{y},{type_id}"
                    )
                    .unwrap();
                }
            }
        }
    }
    (location, marks)
}
