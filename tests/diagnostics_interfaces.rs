use marklab::{AnalysisConfig, AnalysisEngine, MarklabError, Pattern, PatternMeta};

fn meta() -> PatternMeta {
    PatternMeta {
        case_id: "case_001".into(),
        timepoint: "post".into(),
        protein: "MSH6".into(),
        slide_id: None,
        section_id: None,
        stain_batch: None,
        block_id: None,
        region_id: None,
    }
}

fn component_pattern() -> Pattern {
    let mut pattern = Pattern::from_arrays(
        vec![0.0, 1.0, 2.0, 10.0, 11.0, 12.0],
        vec![0.0, 0.0, 0.0, 10.0, 10.0, 10.0],
        vec![1, 1, 0, 1, 0, 0],
        meta(),
    )
    .expect("pattern");
    pattern.component_id = Some(vec![1, 1, 1, 2, 2, 2].into_boxed_slice());
    pattern
}

#[test]
fn analysis_engine_runs_enabled_beta_binomial_diagnostic() {
    let mut config = AnalysisConfig::default();
    config.diagnostics.beta_binomial = true;
    config.permutation.stratified = false;
    config.performance.threads = marklab::ThreadSetting::Count(1);

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&component_pattern())
        .expect("analysis");
    let diagnostics = result.diagnostics.value().expect("diagnostics result");

    assert!(diagnostics.beta_binomial.is_some());
    assert!(diagnostics.graph_smoothing.is_none());
    assert!(result
        .timings
        .iter()
        .any(|stage| stage.stage_name == "diagnostic_beta_binomial"));
}

#[test]
fn analysis_engine_rejects_graph_smoothing_without_multimodal_input() {
    let mut config = AnalysisConfig::default();
    config.diagnostics.graph_smoothing = true;

    let err = match AnalysisEngine::new(config) {
        Ok(_) => panic!("marked-pattern engine should reject graph smoothing"),
        Err(err) => err,
    };

    assert!(matches!(err, MarklabError::Config(_)));
    assert!(err.to_string().contains("graph_smoothing"));
}
