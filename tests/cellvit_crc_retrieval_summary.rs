#![cfg(feature = "cli")]

use std::path::Path;

use assert_cmd::Command;

#[test]
fn crc_retrieval_summary_uses_query_patients_for_effects_and_uncertainty() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("workers/python/marklab_tcga_crc_retrieval_summary.py");
    let program = r#"
import importlib.util
import json
import sys

specification = importlib.util.spec_from_file_location("summary", sys.argv[1])
module = importlib.util.module_from_spec(specification)
specification.loader.exec_module(module)
rows = [
    {"patient_id":"a1","label":"A","matches":[("a2","A",1.0),("b1","B",3.0),("b2","B",4.0)]},
    {"patient_id":"a2","label":"A","matches":[("a1","A",1.0),("b1","B",3.0),("b2","B",4.0)]},
    {"patient_id":"b1","label":"B","matches":[("b2","B",1.0),("a1","A",3.0),("a2","A",4.0)]},
    {"patient_id":"b2","label":"B","matches":[("b1","B",1.0),("a1","A",3.0),("a2","A",4.0)]},
]
result = module.summarize_block(rows, 100, 73)
result["fixed_stage"] = module.common_cohort_value("embedding_stage_ordinal", 2.0)
result["fixed_density"] = module.common_cohort_value("embedding_cell_density_per_mm2", 999.0)
result["fixed_distance"] = module.common_cohort_value("embedding_median_stromal_distance_um", 50.0)
print(json.dumps(result, sort_keys=True, separators=(",", ":")))
"#;
    let assertion = Command::new("python3")
        .args(["-c", program])
        .arg(script)
        .assert()
        .success();
    let summary: serde_json::Value =
        serde_json::from_slice(&assertion.get_output().stdout).unwrap();
    assert_eq!(summary["patient_count"], 4);
    assert_eq!(summary["top1_accuracy"], 1.0);
    assert_eq!(summary["balanced_top1_accuracy"], 1.0);
    assert_eq!(summary["mean_between_minus_within_distance"], 2.5);
    assert_eq!(summary["fixed_stage"], 0.5);
    assert!((summary["fixed_density"].as_f64().unwrap() - 6.907755278982137 / 10.0).abs() < 1e-15);
    assert_eq!(summary["fixed_distance"], 0.5);
    assert_eq!(
        summary["mean_between_minus_within_distance_bootstrap_95"]["lower"],
        2.5
    );
    assert_eq!(
        summary["mean_between_minus_within_distance_bootstrap_95"]["upper"],
        2.5
    );
}
