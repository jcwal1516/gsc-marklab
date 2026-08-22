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

fn dense_geometry_pattern() -> Pattern {
    let side = 20;
    let mut pattern = Pattern::from_arrays(
        (0..side * side)
            .map(|index| (index % side) as f64 * 0.01)
            .collect(),
        (0..side * side)
            .map(|index| (index / side) as f64 * 0.01)
            .collect(),
        (0..side * side)
            .map(|index| u8::from(index % 2 == 0))
            .collect(),
        PatternMeta {
            case_id: "geometry-budget".into(),
            timepoint: "post".into(),
            protein: "MSH6".into(),
            slide_id: None,
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern");
    pattern.window.area_um2 = 100.0;
    pattern.window.analysis_effective_length_um = 10.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern
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

#[test]
fn analysis_engine_rejects_geometry_plans_over_memory_budget() {
    let mut config = contract_config();
    config.performance.memory_budget_mib = 1;
    config.multiscale_residual.enabled = true;
    config.multiscale_residual.territory_detection = true;
    config.multiscale_residual.min_territory_z = 0.1;

    let error = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&dense_geometry_pattern())
        .expect_err("geometry plan should exceed the remaining budget");

    assert!(error
        .to_string()
        .contains("remaining geometry memory budget"));
}

#[test]
fn analysis_telemetry_accounts_for_peak_geometry_storage() {
    let mut config = contract_config();
    config.performance.memory_budget_mib = 8;
    config.multiscale_residual.enabled = true;
    config.multiscale_residual.territory_detection = true;
    config.multiscale_residual.min_territory_z = 0.1;

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&dense_geometry_pattern())
        .expect("analysis within budget");
    let estimated_peak_mib = result
        .timings
        .first()
        .expect("timing")
        .estimated_peak_memory_mib;

    assert!(estimated_peak_mib > 3.0, "{estimated_peak_mib}");
    assert!(estimated_peak_mib <= 8.0, "{estimated_peak_mib}");
    assert!(result
        .timings
        .iter()
        .all(|timing| timing.estimated_peak_memory_mib == estimated_peak_mib));
}
