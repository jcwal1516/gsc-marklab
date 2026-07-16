use marklab::{
    AnalysisConfig, AnalysisEngine, ComponentMode, OutputWriter, Pattern, PatternMeta,
    PermutationStratum, ResultDocument, StatusFlag,
};

fn meta() -> PatternMeta {
    PatternMeta {
        case_id: "case_clustered".into(),
        timepoint: "post".into(),
        protein: "MSH6".into(),
        slide_id: None,
        section_id: None,
        stain_batch: None,
        block_id: None,
        region_id: None,
    }
}

fn permissive_config() -> AnalysisConfig {
    let mut config = AnalysisConfig::default();
    config.validation.n_min = 20;
    config.validation.n_marked_min = 4;
    config.validation.n_unmarked_min = 4;
    config.validation.area_min_um2 = 1.0;
    config.validation.k_shell_min = 3;
    config.validation.valid_mask_fraction_min = 0.5;
    config.spectrum.k_shells = 12;
    config.spectrum.low_k_shells = 3;
    config.permutation.b = 99;
    config.permutation.seed = 17;
    config.permutation.stratified = false;
    config.performance.threads = marklab::ThreadSetting::Count(1);
    config
}

#[test]
fn engine_reports_permutation_whitened_low_k_excess_for_clustered_marks() {
    let mut pattern = Pattern::from_arrays(
        (0..40).map(|value| value as f64).collect(),
        vec![0.0; 40],
        (0..40).map(|index| u8::from(index < 8)).collect(),
        meta(),
    )
    .expect("pattern");
    pattern.window.l_eff_um = 40.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 40.0;

    let engine = AnalysisEngine::new(permissive_config()).expect("engine");
    let result = engine.analyze_pattern(&pattern).expect("analysis");
    let spectrum = result.spectrum.value().expect("spectrum");
    let pair_correlation = result.pair_correlation.value().expect("pair correlation");
    let scalogram = result.scalogram.value().expect("scalogram");
    let wavelet = result.wavelet.value().expect("wavelet");

    assert_eq!(result.status, "ok");
    assert!(spectrum.low_k_excess > 1.5);
    assert!(*result.primary_endpoint.p_value.value().expect("p-value") < 0.10);
    let xi_um = spectrum.xi_um.expect("xi");
    assert!(xi_um > 0.0);
    let xi_interval = result
        .spectrum
        .value()
        .expect("spectrum")
        .xi_stability_interval_um
        .expect("xi stability interval");
    assert!(xi_interval[0] <= xi_um && xi_um <= xi_interval[1]);
    assert!(spectrum.alpha.is_some());
    assert!(
        result
            .spectrum
            .value()
            .expect("spectrum")
            .low_k_excess_p_value
            .expect("low-k scalar p")
            < 0.10
    );
    assert!(spectrum.xi_um_p_value.expect("xi scalar p") > 0.0);
    assert!(spectrum.alpha_p_value.expect("alpha scalar p") > 0.0);
    assert!(spectrum.n_k_modes > spectrum.n_shells);
    assert_eq!(spectrum.n_shells, 12);
    assert!(spectrum.k_min.expect("k_min") > 0.0);
    assert!(spectrum.k_max.expect("k_max") > spectrum.k_min.unwrap());
    assert_eq!(spectrum.n_permutations, 99);
    assert!(spectrum.spectral_curve_test.value().is_some());
    assert!(result.spectrum_curve.iter().all(|point| {
        point.lower_global_envelope <= point.upper_global_envelope
            && point.observed_power >= 0.0
            && point.whitened_power >= 0.0
    }));
    assert_eq!(pair_correlation.n_permutations, 99);
    assert!(pair_correlation.p_global.expect("pair p") > 0.0);
    assert!(pair_correlation.erl_depth.is_some());
    assert!(result
        .pair_correlation_curve
        .iter()
        .all(|point| { point.lower_global_envelope <= point.upper_global_envelope }));
    assert_eq!(scalogram.n_permutations, 99);
    assert!(scalogram.p_global.expect("scalogram p") > 0.0);
    assert!(scalogram.erl_depth.is_some());
    assert!(result
        .scalogram_curve
        .iter()
        .all(|point| { point.lower_global_envelope <= point.upper_global_envelope }));
    assert!(wavelet.coarse_variance_fraction > 0.0);
    assert!(
        wavelet.fine_variance_fraction
            + wavelet.intermediate_variance_fraction
            + wavelet.coarse_variance_fraction
            <= 1.000001
    );
    assert!(wavelet.coarse_to_fine_ratio.expect("ratio").is_finite());
}

#[test]
fn engine_records_the_configured_mark_label_in_results_and_reports() {
    let mut config = permissive_config();
    config.analysis.mark_label = "MMR loss".into();
    config.permutation.b = 99;
    config.inference.family_wise_alpha = 0.25;
    config.output.write_parquet_curves = false;
    config.output.write_geojson_territories = false;
    config.output.write_figures = false;
    config.output.write_run_manifest = false;
    let output = config.output.clone();
    let mut pattern = Pattern::from_arrays(
        (0..40).map(|value| value as f64).collect(),
        vec![0.0; 40],
        (0..40).map(|index| u8::from(index < 8)).collect(),
        meta(),
    )
    .expect("pattern");
    pattern.window.l_eff_um = 40.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 40.0;

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");

    assert_eq!(result.mark_label, "MMR loss");
    let dir = tempfile::tempdir().expect("output directory");
    OutputWriter::write(&ResultDocument::marked(result), dir.path(), &output)
        .expect("write outputs");
    let report = std::fs::read_to_string(dir.path().join("report.md")).expect("report");
    assert!(report.contains("Mark label: MMR loss"));
    assert!(report.contains("8 MMR loss"));
}

#[test]
fn engine_uses_fixed_one_sided_high_test_for_low_k_excess() {
    let mut pattern = Pattern::from_arrays(
        (0..40).map(|value| value as f64).collect(),
        vec![0.0; 40],
        (0..40).map(|index| u8::from(index < 8)).collect(),
        meta(),
    )
    .expect("pattern");
    pattern.window.l_eff_um = 40.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 40.0;

    let result = AnalysisEngine::new(permissive_config())
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");

    let spectrum = result.spectrum.value().expect("spectrum");
    assert!(spectrum.low_k_excess > 1.5);
    assert!(
        result
            .spectrum
            .value()
            .expect("spectrum")
            .low_k_excess_p_value
            .expect("high-tail scalar p")
            < 0.10
    );
}

#[test]
fn engine_marks_low_k_suppression_when_low_frequency_power_is_below_permutations() {
    let mut pattern = Pattern::from_arrays(
        (0..40).map(|value| value as f64).collect(),
        vec![0.0; 40],
        (0..40).map(|index| u8::from(index % 5 == 0)).collect(),
        meta(),
    )
    .expect("pattern");
    pattern.window.l_eff_um = 40.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 40.0;

    let engine = AnalysisEngine::new(permissive_config()).expect("engine");
    let result = engine.analyze_pattern(&pattern).expect("analysis");

    assert!(result.spectrum.value().expect("spectrum").low_k_excess < 1.0);
    assert_eq!(result.interpretation.class, "low_k_suppressed_or_dispersed");
}

#[test]
fn engine_omits_alpha_when_low_k_alpha_fit_is_disabled() {
    let mut config = permissive_config();
    config.spectrum.fit_low_k_alpha = false;
    let mut pattern = Pattern::from_arrays(
        (0..40).map(|value| value as f64).collect(),
        vec![0.0; 40],
        (0..40).map(|index| u8::from(index < 8)).collect(),
        meta(),
    )
    .expect("pattern");
    pattern.window.l_eff_um = 40.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 40.0;

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");

    let spectrum = result.spectrum.value().expect("spectrum");
    assert!(spectrum.alpha.is_none());
    assert!(spectrum.alpha_p_value.is_none());
}

#[test]
fn engine_omits_wavelet_outputs_when_wavelet_is_disabled() {
    let mut config = permissive_config();
    config.wavelet.enabled = false;
    config.wavelet.territory_detection = true;
    let mut pattern = Pattern::from_arrays(
        (0..40).map(|value| value as f64).collect(),
        vec![0.0; 40],
        (0..40).map(|index| u8::from(index < 8)).collect(),
        meta(),
    )
    .expect("pattern");
    pattern.window.l_eff_um = 40.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 40.0;

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");

    assert!(matches!(result.wavelet, marklab::AnalysisSection::Disabled));
    assert!(result.scalogram_curve.is_empty());
    assert!(matches!(
        result.scalogram,
        marklab::AnalysisSection::Disabled
    ));
    assert!(matches!(
        result.wavelet_territories,
        marklab::AnalysisSection::Disabled
    ));
}

#[test]
fn engine_omits_territories_when_territory_detection_is_disabled() {
    let mut config = permissive_config();
    config.wavelet.enabled = true;
    config.wavelet.territory_detection = false;
    let mut pattern = Pattern::from_arrays(
        (0..40).map(|value| value as f64).collect(),
        vec![0.0; 40],
        (0..40).map(|index| u8::from(index < 8)).collect(),
        meta(),
    )
    .expect("pattern");
    pattern.window.l_eff_um = 40.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 40.0;

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");

    assert_eq!(result.wavelet.value().expect("wavelet").territory_count, 0);
    assert!(matches!(
        result.wavelet_territories,
        marklab::AnalysisSection::Disabled
    ));
    assert!(
        result
            .wavelet
            .value()
            .expect("wavelet")
            .coarse_variance_fraction
            > 0.0
    );
    assert!(!result.scalogram_curve.is_empty());
}

#[test]
fn engine_marks_out_of_range_wavelet_endpoints_insufficient() {
    let mut config = permissive_config();
    config.permutation.b = 19;
    config.inference.family_wise_alpha = 0.25;
    config.validation.largest_interpretable_scale_fraction = 0.05;
    let mut pattern = Pattern::from_arrays(
        (0..40).map(|value| value as f64).collect(),
        vec![0.0; 40],
        (0..40).map(|index| u8::from(index < 8)).collect(),
        meta(),
    )
    .expect("pattern");
    pattern.window.l_eff_um = 40.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 40.0;

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");
    let wavelet = result.wavelet.value().expect("wavelet");

    assert!(matches!(
        wavelet.coarse_variance_fraction_p_value,
        marklab::AnalysisSection::InsufficientData { .. }
    ));
    assert!(matches!(
        wavelet.territory_count_p_value,
        marklab::AnalysisSection::Available { .. }
    ));
}

#[test]
fn engine_flags_confounding_when_cluster_is_explained_by_qc_strata() {
    let mut config = permissive_config();
    config.inference.family_wise_alpha = 0.10;
    config.permutation.stratified = true;
    config.permutation.strata_fields = vec![PermutationStratum::QcBin];

    let mut pattern = Pattern::from_arrays(
        (0..40).map(|value| value as f64).collect(),
        vec![0.0; 40],
        (0..40).map(|index| u8::from(index < 8)).collect(),
        meta(),
    )
    .expect("pattern");
    pattern.window.l_eff_um = 40.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 40.0;
    pattern.qc_bin = Some(
        (0..40)
            .map(|index| if index < 8 { 1_u16 } else { 2_u16 })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );

    let engine = AnalysisEngine::new(config).expect("engine");
    let result = engine.analyze_pattern(&pattern).expect("analysis");

    assert_eq!(
        result
            .spectrum
            .value()
            .expect("spectrum")
            .low_k_excess_p_value,
        Some(1.0),
        "the configured stratified null must be the primary spectrum null"
    );
    assert_eq!(
        result
            .pair_correlation
            .value()
            .and_then(|summary| summary.p_global),
        Some(1.0),
        "pair-correlation inference must use the configured stratified null"
    );
    assert!(
        matches!(
            result.anisotropy,
            marklab::AnalysisSection::InsufficientData { .. }
        ),
        "a degenerate stratified anisotropy null must be reported as insufficient"
    );
    assert_eq!(
        result
            .wavelet
            .value()
            .and_then(|summary| summary.coarse_variance_fraction_p_value.value())
            .copied(),
        Some(1.0),
        "wavelet inference must use the configured stratified null"
    );
    assert!(
        result
            .status_flags
            .contains(&StatusFlag::ConfoundedBySpatialStrata),
        "flags={:?}, spectral_curve_test={:?}",
        result.status_flags,
        result
            .spectrum
            .value()
            .map(|value| &value.spectral_curve_test)
    );
    assert_eq!(result.status, "suppressed");
}

#[test]
fn engine_can_stratify_by_component_id_when_qc_bin_is_absent() {
    let mut config = permissive_config();
    config.inference.family_wise_alpha = 0.10;
    config.permutation.stratified = true;
    config.permutation.strata_fields = vec![PermutationStratum::ComponentId];

    let mut pattern = Pattern::from_arrays(
        (0..40).map(|value| value as f64).collect(),
        vec![0.0; 40],
        (0..40).map(|index| u8::from(index < 8)).collect(),
        meta(),
    )
    .expect("pattern");
    pattern.window.l_eff_um = 40.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 40.0;
    pattern.component_id = Some(
        (0..40)
            .map(|index| if index < 8 { 100_u32 } else { 200_u32 })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );

    let engine = AnalysisEngine::new(config).expect("engine");
    let result = engine.analyze_pattern(&pattern).expect("analysis");

    assert!(
        result
            .status_flags
            .contains(&StatusFlag::ConfoundedBySpatialStrata),
        "flags={:?}, p_value={:?}",
        result.status_flags,
        result.primary_endpoint.p_value
    );
}

#[test]
fn engine_reports_separate_component_summaries_when_component_mode_is_both() {
    let mut config = permissive_config();
    config.analysis.analyze_components = ComponentMode::Both;
    config.validation.n_min = 6;
    config.validation.n_marked_min = 1;
    config.validation.n_unmarked_min = 1;
    config.validation.k_shell_min = 1;
    config.spectrum.k_shells = 8;
    config.permutation.b = 19;
    config.inference.family_wise_alpha = 0.25;

    let mut pattern = Pattern::from_arrays(
        (0..24)
            .map(|index| {
                if index < 12 {
                    index as f64
                } else {
                    100.0 + index as f64
                }
            })
            .collect(),
        vec![0.0; 24],
        (0..24)
            .map(|index| u8::from(index < 4 || (12..16).contains(&index)))
            .collect(),
        meta(),
    )
    .expect("pattern");
    pattern.window.l_eff_um = 124.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 124.0;
    pattern.component_id = Some(
        (0..24)
            .map(|index| if index < 12 { 10_u32 } else { 20_u32 })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");

    assert_eq!(result.n_cells, 24);
    let components = result.component_results.value().expect("components");
    assert_eq!(components.len(), 2);
    let first = result
        .component_results
        .value()
        .expect("components")
        .iter()
        .find(|component| component.component_id == 10)
        .expect("component 10");
    assert_eq!(first.n_cells, 12);
    assert_eq!(first.n_marked, 4);
    assert!((first.p_hat - (4.0 / 12.0)).abs() < 1e-12);
    assert!(matches!(
        first.primary_endpoint_value,
        marklab::AnalysisSection::Available { .. }
            | marklab::AnalysisSection::InsufficientData { .. }
    ));
    if first.primary_endpoint_value.value().is_some() {
        assert!(first.k_min.expect("component k_min") > 0.0);
        assert!(first.k_max.expect("component k_max") > first.k_min.unwrap());
    } else {
        assert!(first.k_min.is_none());
        assert!(first.k_max.is_none());
    }

    let pooled_config = {
        let mut config = permissive_config();
        config.analysis.analyze_components = ComponentMode::Pooled;
        config.validation.n_min = 6;
        config.validation.n_marked_min = 1;
        config.validation.n_unmarked_min = 1;
        config
    };
    let pooled = AnalysisEngine::new(pooled_config)
        .expect("pooled engine")
        .analyze_pattern(&pattern)
        .expect("pooled analysis");
    assert!(pooled.component_results.value().is_some_and(Vec::is_empty));
}

#[test]
fn component_summaries_use_the_configured_interpretable_scale() {
    let mut config = permissive_config();
    config.analysis.analyze_components = ComponentMode::Both;
    config.validation.n_min = 64;
    config.validation.n_marked_min = 8;
    config.validation.n_unmarked_min = 8;
    config.validation.k_shell_min = 1;
    config.validation.largest_interpretable_scale_fraction = 0.01;
    config.spectrum.k_shells = 8;
    config.permutation.b = 99;
    config.inference.family_wise_alpha = 0.25;

    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut marks = Vec::new();
    let mut component_ids = Vec::new();
    for component in 0..2 {
        for row in 0..8 {
            for column in 0..8 {
                x.push(column as f64 + component as f64 * 100.0);
                y.push(row as f64);
                marks.push(u8::from(row < 2 && column < 4));
                component_ids.push(10 + component * 10);
            }
        }
    }
    let mut pattern = Pattern::from_arrays(x, y, marks, meta()).expect("pattern");
    pattern.window.l_eff_um = 108.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 128.0;
    pattern.component_id = Some(component_ids.into_boxed_slice());

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");
    let components = result.component_results.value().expect("components");

    assert_eq!(components.len(), 2);
    assert!(components.iter().all(|component| matches!(
        component.primary_endpoint_value,
        marklab::AnalysisSection::InsufficientData { .. }
    )));
}

#[test]
fn engine_reports_permutation_whitened_anisotropy_for_oriented_pattern() {
    let mut config = permissive_config();
    config.validation.n_min = 100;
    config.validation.n_marked_min = 5;
    config.validation.n_unmarked_min = 20;
    config.spectrum.k_shells = 10;
    config.spectrum.low_k_shells = 3;
    config.spectrum.anisotropy_low_k_shells = 3;
    config.permutation.b = 49;

    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut marks = Vec::new();
    for row in 0..10 {
        for col in 0..20 {
            x.push(col as f64);
            y.push(row as f64);
            marks.push(u8::from(col == 0));
        }
    }

    let mut pattern = Pattern::from_arrays(x, y, marks, meta()).expect("pattern");
    pattern.window.l_eff_um = 20.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 200.0;

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");

    let anisotropy = result.anisotropy.value().expect("anisotropy");
    assert!(anisotropy.index > 1.2);
    assert!(anisotropy.theta_deg.is_some());
    assert!(anisotropy.p_value.expect("anisotropy p") < 0.20);
}

#[test]
fn engine_uses_probabilistic_marks_when_configured() {
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut marks = Vec::new();
    let mut probabilities = Vec::new();
    for row in 0..10 {
        for col in 0..10 {
            x.push(col as f64);
            y.push(row as f64);
            marks.push(u8::from((row + col) % 2 == 0));
            probabilities.push(if col < 5 { 0.90 } else { 0.10 });
        }
    }

    let mut pattern = Pattern::from_arrays(x, y, marks, meta()).expect("pattern");
    pattern.mark_prob = Some(probabilities.into_boxed_slice());
    pattern.window.l_eff_um = 10.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 100.0;

    let mut binary_config = permissive_config();
    binary_config.validation.n_min = 100;
    binary_config.validation.n_marked_min = 10;
    binary_config.validation.n_unmarked_min = 10;
    binary_config.spectrum.k_shells = 8;
    binary_config.spectrum.low_k_shells = 2;
    binary_config.permutation.b = 49;

    let mut probability_config = binary_config.clone();
    probability_config.analysis.use_probabilistic_marks = true;

    let binary = AnalysisEngine::new(binary_config)
        .expect("binary engine")
        .analyze_pattern(&pattern)
        .expect("binary analysis");
    let probabilistic = AnalysisEngine::new(probability_config)
        .expect("probability engine")
        .analyze_pattern(&pattern)
        .expect("probability analysis");

    assert!(
        probabilistic
            .spectrum
            .value()
            .expect("probabilistic spectrum")
            .low_k_excess
            > binary
                .spectrum
                .value()
                .expect("binary spectrum")
                .low_k_excess
                * 1.25
    );
    let p_value = *probabilistic.primary_endpoint.p_value.value().expect("p");
    assert!(0.0 < p_value && p_value <= 1.0);
}

#[test]
fn engine_records_permutation_stage_when_probabilistic_marks_are_missing() {
    let mut pattern = Pattern::from_arrays(
        (0..20).map(|index| index as f64).collect(),
        vec![0.0; 20],
        (0..20).map(|index| u8::from(index % 2 == 0)).collect(),
        meta(),
    )
    .expect("pattern");
    pattern.window.l_eff_um = 20.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 100.0;

    let mut config = permissive_config();
    config.analysis.use_probabilistic_marks = true;
    config.validation.n_min = 20;
    config.validation.n_marked_min = 4;
    config.validation.n_unmarked_min = 4;
    config.validation.k_shell_min = 1;
    config.spectrum.k_shells = 8;
    config.spectrum.low_k_shells = 2;
    config.permutation.b = 9;
    config.inference.family_wise_alpha = 0.25;

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");

    assert!(result
        .timings
        .iter()
        .any(|stage| stage.stage_name == "permutation_spectra"));
    assert!(matches!(
        result.spectrum,
        marklab::AnalysisSection::InsufficientData { .. }
    ));
}

#[test]
fn engine_marks_spectrum_insufficient_when_all_scales_are_out_of_range() {
    let mut config = permissive_config();
    config.validation.n_min = 64;
    config.validation.n_marked_min = 8;
    config.validation.n_unmarked_min = 8;
    config.validation.k_shell_min = 1;
    config.spectrum.k_shells = 5;
    config.spectrum.low_k_shells = 2;
    config.permutation.b = 19;
    config.inference.family_wise_alpha = 0.25;
    config.periodogram.enabled = true;

    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut marks = Vec::new();
    for row in 0..8 {
        for col in 0..8 {
            x.push(col as f64);
            y.push(row as f64);
            marks.push(u8::from(row < 4 && col < 4));
        }
    }

    let mut pattern = Pattern::from_arrays(x, y, marks, meta()).expect("pattern");
    pattern.window.l_eff_um = 8.0;
    pattern.window.d_nn_mean_um = 8.0;
    pattern.window.area_um2 = 64.0;

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");

    assert!(matches!(
        result.spectrum,
        marklab::AnalysisSection::InsufficientData { .. }
    ));
    assert!(!result
        .status_flags
        .contains(&StatusFlag::WindowOrGriddingArtifactSuspect));
}

#[test]
fn engine_detects_multiple_residual_territory_maxima() {
    let mut config = permissive_config();
    config.validation.n_min = 100;
    config.validation.n_marked_min = 4;
    config.validation.n_unmarked_min = 20;
    config.validation.k_shell_min = 1;
    config.spectrum.k_shells = 8;
    config.spectrum.low_k_shells = 2;
    config.permutation.b = 19;
    config.inference.family_wise_alpha = 0.25;
    config.wavelet.enabled = true;
    config.wavelet.territory_detection = true;
    config.wavelet.min_territory_z = 2.0;

    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut marks = Vec::new();
    for row in 0..10 {
        for col in 0..10 {
            x.push(col as f64);
            y.push(row as f64);
            let in_left_focus = row <= 1 && col <= 1;
            let in_right_focus = row >= 8 && col >= 8;
            marks.push(u8::from(in_left_focus || in_right_focus));
        }
    }

    let mut pattern = Pattern::from_arrays(x, y, marks, meta()).expect("pattern");
    pattern.window.l_eff_um = 10.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 100.0;

    let result = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect("analysis");

    let territories = result
        .wavelet_territories
        .value()
        .expect("wavelet territories");
    assert!(territories.len() >= 2, "{territories:?}");
    assert!(
        *result
            .wavelet
            .value()
            .expect("wavelet")
            .coarse_variance_fraction_p_value
            .value()
            .expect("coarse variance p-value")
            > 0.0
    );
    assert!(
        *result
            .wavelet
            .value()
            .expect("wavelet")
            .territory_count_p_value
            .value()
            .expect("territory count p-value")
            > 0.0
    );
    assert!(result
        .wavelet_territories
        .value()
        .expect("wavelet territories")
        .iter()
        .any(|territory| territory.center_x_um < 3.0 && territory.center_y_um < 3.0));
    assert!(result
        .wavelet_territories
        .value()
        .expect("wavelet territories")
        .iter()
        .any(|territory| territory.center_x_um > 6.0 && territory.center_y_um > 6.0));
}
