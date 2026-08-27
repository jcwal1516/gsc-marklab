#![cfg(feature = "cli")]

use std::path::Path;

use assert_cmd::Command;

#[test]
fn cellvit_adapter_builds_sorted_neoplastic_successes_and_all_cell_trials() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("workers/python/marklab_cellvit_cptac_results_adapter.py");
    let program = r#"
import importlib.util
import json
import sys
from collections import Counter

specification = importlib.util.spec_from_file_location("adapter", sys.argv[1])
module = importlib.util.module_from_spec(specification)
specification.loader.exec_module(module)
rows = module.beta_binomial_patient_rows(
    {
        "patient-b": Counter({1: 2, 2: 3}),
        "patient-a": Counter({1: 7, 3: 1}),
        "patient-c": Counter({0: 4, 4: 6}),
    },
    ((0, "Background"), (1, "Neoplastic"), (2, "Inflammatory"), (3, "Connective"), (4, "Dead")),
)
print(json.dumps(rows, sort_keys=True, separators=(",", ":")))
"#;
    let assertion = Command::new("python3")
        .args(["-c", program])
        .arg(script)
        .assert()
        .success();
    let rows: serde_json::Value =
        serde_json::from_slice(&assertion.get_output().stdout).expect("row JSON");
    assert_eq!(
        rows,
        serde_json::json!([
            {"patient_id": "patient-a", "successes": 7, "trials": 8},
            {"patient_id": "patient-b", "successes": 2, "trials": 5},
            {"patient_id": "patient-c", "successes": 0, "trials": 10}
        ])
    );
}
