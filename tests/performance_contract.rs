use marklab::{AnalysisConfig, AnalysisEngine, Pattern, PatternMeta, ThreadSetting};

fn contract_config() -> AnalysisConfig {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 8;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.area_min_um2 = 1.0;
    config.validation.k_shell_min = 1;
    config.spectrum.k_shells = 4;
    config.spectrum.low_k_shells = 2;
    config.spectrum.anisotropy_low_k_shells = 3;
    config.permutation.b = 17;
    config.permutation.seed = 777;
    config.permutation.stratified = false;
    config.inference.family_wise_alpha = 0.25;
    config.periodogram.enabled = false;
    config.multiscale_residual.enabled = false;
    config.performance.memory_budget_mib = 128;
    config
}

fn contract_pattern() -> Pattern {
    Pattern::from_arrays(
        vec![0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        vec![1, 0, 0, 1, 0, 1, 0, 0],
        PatternMeta {
            case_id: "case_parallel_repro".into(),
            timepoint: "post".into(),
            protein: "MSH6".into(),
            slide_id: None,
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern")
}

#[test]
fn analysis_engine_honors_user_thread_count() {
    let mut config = contract_config();
    config.performance.threads = ThreadSetting::Count(2);

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&contract_pattern())
        .expect("analysis");

    assert!(result.timings.iter().all(|stage| stage.cpu_threads == 2));
}

#[test]
fn strict_repro_forces_fixed_order_single_thread_execution() {
    let mut config = contract_config();
    config.performance.threads = ThreadSetting::Count(8);
    config.performance.strict_repro = true;

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&contract_pattern())
        .expect("analysis");

    assert!(result.timings.iter().all(|stage| stage.cpu_threads == 1));
}

#[test]
fn permutation_spectrum_is_reproducible_across_thread_counts() {
    let mut single_thread = contract_config();
    single_thread.performance.threads = ThreadSetting::Count(1);

    let mut two_threads = single_thread.clone();
    two_threads.performance.threads = ThreadSetting::Count(2);

    let pattern = contract_pattern();

    let first = AnalysisEngine::new(single_thread)
        .expect("single-thread engine")
        .analyze_pattern(&pattern)
        .expect("single-thread result");
    let second = AnalysisEngine::new(two_threads)
        .expect("two-thread engine")
        .analyze_pattern(&pattern)
        .expect("two-thread result");

    assert_eq!(first.spectrum_curve, second.spectrum_curve);
    assert_eq!(first.spectrum, second.spectrum);
    assert_eq!(
        first.primary_endpoint.p_value,
        second.primary_endpoint.p_value
    );
}

#[test]
fn analysis_engine_rejects_a_zero_memory_budget_at_configuration_time() {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 4;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.performance.memory_budget_mib = 0;
    let err = AnalysisEngine::new(config)
        .err()
        .expect("zero memory budget should be invalid");

    assert!(err.to_string().contains("memory_budget_mib"));
}
