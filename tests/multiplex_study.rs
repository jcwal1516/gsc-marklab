#[path = "support/multiplex_study.rs"]
mod support;

use marklab::analyze_multiplex_study;
use marklab_cohort::{
    max_t_multiple_endpoint_permutation, MaxTPermutationSpec, PatientEndpointVector,
};

#[test]
fn a_panel_study_reduces_slides_to_independent_patients_and_adjusts_the_complete_family() {
    let bytes = serde_json::to_vec(&support::recipe()).unwrap();
    let result = analyze_multiplex_study(&bytes).unwrap();
    let document = serde_json::to_value(&result).unwrap();
    assert_eq!(document["slides"].as_array().unwrap().len(), 12);
    let patients = document["patients"].as_array().unwrap();
    assert_eq!(patients.len(), 6);
    let names = document["endpoint_names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        ["CD3:moran_i", "CD3:geary_c", "CD8:moran_i", "CD8:geary_c"]
    );
    let vectors = patients
        .iter()
        .map(|patient| {
            assert_eq!(patient["slide_count"], 2);
            PatientEndpointVector {
                patient_id: patient["patient_id"].as_str().unwrap().into(),
                group: patient["group"].as_str().unwrap().into(),
                endpoints: names.clone(),
                values: patient["values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_f64().unwrap())
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    let reference = max_t_multiple_endpoint_permutation(
        &vectors,
        &MaxTPermutationSpec {
            group_a: "comparison".into(),
            group_b: "reference".into(),
            permutations: 99,
            seed: 41,
            alpha: 0.05,
        },
    )
    .unwrap();
    for (row, expected) in document["inference"]["endpoints"]
        .as_array()
        .unwrap()
        .iter()
        .zip(&reference.endpoints)
    {
        assert_eq!(
            row["adjusted_p_value"].as_f64().unwrap(),
            expected.adjusted_p_value
        );
        assert_eq!(
            row["effect_group_a_minus_group_b"].as_f64().unwrap(),
            expected.effect_group_a_minus_group_b
        );
    }
    assert_eq!(document["maturity"]["result_maturity"], "experimental");
    assert_eq!(document["inference"]["status"], "available");
    assert_eq!(
        document["channels"][3]["measurement_status"],
        "imported_prediction"
    );
}

#[test]
fn one_unavailable_slide_prevents_silent_patient_or_endpoint_selection() {
    let mut recipe = support::recipe();
    recipe["slides"][0]["observations"]["CD3"] =
        serde_json::json!([null, null, null, null, null, null]);
    let result = analyze_multiplex_study(&serde_json::to_vec(&recipe).unwrap()).unwrap();
    let document = serde_json::to_value(result).unwrap();
    assert_eq!(document["slides"].as_array().unwrap().len(), 12);
    assert_eq!(document["patients"].as_array().unwrap().len(), 6);
    assert_eq!(document["inference"]["status"], "unavailable");
    assert_eq!(document["patients"][0]["status"], "unavailable");
    assert_eq!(
        document["maturity"]["result_maturity"],
        "unsupported_for_claim"
    );
}

#[test]
fn malformed_and_unbounded_recipes_fail_before_analysis() {
    for change in [
        "unknown",
        "identity",
        "unit",
        "selection",
        "budget",
        "group",
        "cross_slide_cell_alias",
    ] {
        let mut recipe = support::recipe();
        match change {
            "unknown" => recipe["silent_imputation"] = true.into(),
            "identity" => recipe["slides"][0]["cell_ids"][1] = "cell-0".into(),
            "unit" => recipe["channels"][2]["unit"] = "um".into(),
            "selection" => recipe["design"]["selected_channels"][0] = "cell_type".into(),
            "budget" => recipe["limits"] = serde_json::json!({"maximum_memory_bytes":1}),
            "group" => recipe["slides"][0]["group"] = "undeclared".into(),
            "cross_slide_cell_alias" => {
                recipe["slides"][1]["cell_ids"] = recipe["slides"][0]["cell_ids"].clone()
            }
            _ => unreachable!(),
        }
        assert!(
            analyze_multiplex_study(&serde_json::to_vec(&recipe).unwrap()).is_err(),
            "{change}"
        );
    }
}

fn runtime() -> marklab::NativeRuntimeProvenance {
    marklab::NativeRuntimeProvenance::new(
        "0.1.0",
        None,
        None,
        "test-rustc",
        vec!["test".into()],
        marklab::ArtifactRef::from_bytes(
            "application/vnd.marklab.executable",
            b"multiplex-test-client",
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn a_partial_study_resumes_without_reexecuting_completed_slide_nodes() {
    use marklab::{execute_multiplex_study, MultiplexStudyTarget};
    let root = tempfile::tempdir().unwrap();
    let bytes = serde_json::to_vec(&support::recipe()).unwrap();
    let partial = execute_multiplex_study(
        root.path(),
        &bytes,
        MultiplexStudyTarget::ThroughSlides(3),
        runtime(),
    )
    .unwrap();
    assert!(partial.result.is_none());
    assert_eq!(partial.executed_slides, 3);
    assert_eq!(partial.durable_execution_count, 3);
    let completed = execute_multiplex_study(
        root.path(),
        &bytes,
        MultiplexStudyTarget::Complete,
        runtime(),
    )
    .unwrap();
    assert_eq!(completed.restored_slides, 3);
    assert_eq!(completed.executed_slides, 9);
    assert_eq!(completed.durable_execution_count, 13);
    let replay = execute_multiplex_study(
        root.path(),
        &bytes,
        MultiplexStudyTarget::Complete,
        runtime(),
    )
    .unwrap();
    assert_eq!(replay.executed_slides, 0);
    assert_eq!(replay.restored_slides, 12);
    assert_eq!(replay.durable_execution_count, 13);
    let expected = serde_json::to_vec(&analyze_multiplex_study(&bytes).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_vec(&completed.result.unwrap()).unwrap(),
        expected
    );
    assert_eq!(
        serde_json::to_vec(&replay.result.unwrap()).unwrap(),
        expected
    );
}

#[test]
fn changed_slide_content_invalidates_only_that_slide_and_patient_inference() {
    use marklab::{execute_multiplex_study, MultiplexStudyTarget};
    let root = tempfile::tempdir().unwrap();
    let mut recipe = support::recipe();
    execute_multiplex_study(
        root.path(),
        &serde_json::to_vec(&recipe).unwrap(),
        MultiplexStudyTarget::Complete,
        runtime(),
    )
    .unwrap();
    recipe["slides"][0]["observations"]["CD3"][1] = 2.25.into();
    let changed = execute_multiplex_study(
        root.path(),
        &serde_json::to_vec(&recipe).unwrap(),
        MultiplexStudyTarget::Complete,
        runtime(),
    )
    .unwrap();
    assert_eq!(changed.executed_slides, 1);
    assert_eq!(changed.restored_slides, 11);
    assert_eq!(changed.durable_execution_count, 15);
}

#[test]
fn a_study_with_more_than_sixty_four_slides_uses_bounded_dependency_collections() {
    use marklab::{execute_multiplex_study, MultiplexStudyTarget};
    let root = tempfile::tempdir().unwrap();
    let mut recipe = support::recipe();
    let originals = recipe["slides"].as_array().unwrap().clone();
    let slides = recipe["slides"].as_array_mut().unwrap();
    for index in 12..65 {
        let mut slide = originals[index % originals.len()].clone();
        slide["slide_id"] = format!("extra-{index:03}").into();
        slide["coordinate_frame_id"] = format!("extra-{index:03}-um").into();
        slide["cell_ids"] = serde_json::json!((0..6)
            .map(|row| format!("extra-{index:03}:cell-{row}"))
            .collect::<Vec<_>>());
        slides.push(slide);
    }
    let bytes = serde_json::to_vec(&recipe).unwrap();
    let run = execute_multiplex_study(
        root.path(),
        &bytes,
        MultiplexStudyTarget::Complete,
        runtime(),
    )
    .unwrap();
    assert_eq!(run.executed_slides, 65);
    assert_eq!(run.durable_execution_count, 68);
    assert_eq!(
        serde_json::to_vec(&run.result.unwrap()).unwrap(),
        serde_json::to_vec(&analyze_multiplex_study(&bytes).unwrap()).unwrap()
    );
    let replay = execute_multiplex_study(
        root.path(),
        &bytes,
        MultiplexStudyTarget::Complete,
        runtime(),
    )
    .unwrap();
    assert_eq!(replay.restored_slides, 65);
    assert_eq!(replay.durable_execution_count, 68);
}

#[test]
fn a_claim_bounded_report_is_published_atomically_without_overwriting_user_files() {
    let root = tempfile::tempdir().unwrap();
    let out = root.path().join("report");
    let result = analyze_multiplex_study(&serde_json::to_vec(&support::recipe()).unwrap()).unwrap();
    marklab::publish_multiplex_study(&result, &out).unwrap();
    assert!(out.join("result.json").is_file());
    let report = std::fs::read_to_string(out.join("report.md")).unwrap();
    assert!(report.contains("6 independent patients"));
    assert!(report.contains("experimental"));
    assert!(report.contains("Missingness"));
    let before = std::fs::read(out.join("result.json")).unwrap();
    assert!(marklab::publish_multiplex_study(&result, &out).is_err());
    assert_eq!(std::fs::read(out.join("result.json")).unwrap(), before);
}

#[test]
fn a_deserialized_result_cannot_publish_an_inference_with_missing_statistics() {
    let root = tempfile::tempdir().unwrap();
    let result = analyze_multiplex_study(&serde_json::to_vec(&support::recipe()).unwrap()).unwrap();
    let mut wire = serde_json::to_value(result).unwrap();
    wire["inference"]["endpoints"][0]["adjusted_p_value"] = (-0.1).into();
    let invalid: marklab::MultiplexStudyResult = serde_json::from_value(wire).unwrap();
    assert!(marklab::publish_multiplex_study(&invalid, &root.path().join("invalid")).is_err());
    assert!(!root.path().join("invalid").exists());
}

#[test]
fn constant_and_unrepresentable_variances_have_distinct_diagnostics() {
    let mut observed = Vec::new();
    for (values, reason) in [
        (
            vec![1e308, -1e308, 1e308, -1e308, 1e308, -1e308],
            "numerical_failure",
        ),
        (
            vec![1e-308, -1e-308, 1e-308, -1e-308, 1e-308, -1e-308],
            "numerical_failure",
        ),
        (vec![0.1; 6], "zero_variance"),
        (vec![1e308; 6], "zero_variance"),
    ] {
        let mut recipe = support::recipe();
        recipe["slides"][0]["observations"]["CD3"] = serde_json::json!(values);
        let output = serde_json::to_value(
            analyze_multiplex_study(&serde_json::to_vec(&recipe).unwrap()).unwrap(),
        )
        .unwrap();
        observed.push((
            output["slides"][0]["channels"][0]["reason"].clone(),
            output["inference"]["status"].clone(),
            reason,
        ));
    }
    for (actual_reason, status, expected_reason) in &observed {
        assert_eq!(actual_reason, expected_reason, "all cases: {observed:?}");
        assert_eq!(status, "unavailable");
    }
}
