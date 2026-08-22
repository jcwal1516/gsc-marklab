use marklab::{
    AnalysisConfig, AnalysisEngine, AnalysisSection, ComponentMode, OutputWriter, Pattern,
    PatternMeta, PermutationStratum, ResolvedComponentMode, ResultDocument,
    SpectrumConfoundingConclusion, SpectrumNullModel, StatusFlag,
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
    let mark_pair_covariance = result
        .mark_pair_covariance
        .value()
        .expect("mark-pair covariance");
    let scale_energy = result.scale_energy.value().expect("scale energy");
    let multiscale_residual = result
        .multiscale_residual
        .value()
        .expect("multiscale residual");

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
    assert_eq!(mark_pair_covariance.n_permutations, 99);
    assert!(mark_pair_covariance.p_global.expect("pair p") > 0.0);
    assert!(mark_pair_covariance.erl_depth.is_some());
    assert!(result
        .mark_pair_covariance_curve
        .iter()
        .all(|point| { point.lower_global_envelope <= point.upper_global_envelope }));
    assert_eq!(scale_energy.n_permutations, 99);
    assert!(scale_energy.p_global.expect("scale-energy p") > 0.0);
    assert!(scale_energy.erl_depth.is_some());
    assert!(result
        .scale_energy_curve
        .iter()
        .all(|point| { point.lower_global_envelope <= point.upper_global_envelope }));
    assert!(multiscale_residual.block_mean_variance_fraction > 0.0);
    assert!(
        multiscale_residual.local_difference_energy_fraction
            + multiscale_residual.residual_energy_fraction
            + multiscale_residual.block_mean_variance_fraction
            <= 1.000001
    );
    assert!(multiscale_residual
        .block_mean_to_local_difference_ratio
        .expect("ratio")
        .is_finite());
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
    assert_eq!(result.interpretation.class, "low_frequency_suppression");
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
fn engine_omits_multiscale_residual_outputs_when_multiscale_residual_is_disabled() {
    let mut config = permissive_config();
    config.multiscale_residual.enabled = false;
    config.multiscale_residual.territory_detection = true;
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

    assert!(matches!(
        result.multiscale_residual,
        marklab::AnalysisSection::Disabled
    ));
    assert!(result.scale_energy_curve.is_empty());
    assert!(matches!(
        result.scale_energy,
        marklab::AnalysisSection::Disabled
    ));
    assert!(matches!(
        result.residual_territories,
        marklab::AnalysisSection::Disabled
    ));
}

#[test]
fn engine_omits_territories_when_territory_detection_is_disabled() {
    let mut config = permissive_config();
    config.multiscale_residual.enabled = true;
    config.multiscale_residual.territory_detection = false;
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

    assert_eq!(
        result
            .multiscale_residual
            .value()
            .expect("multiscale_residual")
            .territory_count,
        0
    );
    assert!(matches!(
        result.residual_territories,
        marklab::AnalysisSection::Disabled
    ));
    assert!(
        result
            .multiscale_residual
            .value()
            .expect("multiscale_residual")
            .block_mean_variance_fraction
            > 0.0
    );
    assert!(!result.scale_energy_curve.is_empty());
}

#[test]
fn engine_marks_out_of_range_multiscale_residual_endpoints_insufficient() {
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
    let multiscale_residual = result
        .multiscale_residual
        .value()
        .expect("multiscale_residual");

    assert!(matches!(
        multiscale_residual.block_mean_variance_fraction_p_value,
        marklab::AnalysisSection::InsufficientData { .. }
    ));
    assert!(matches!(
        multiscale_residual.territory_count_p_value,
        marklab::AnalysisSection::Available { .. }
    ));
}

#[test]
fn homogeneous_strata_report_degenerate_null() {
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

    assert!(
        matches!(
            &result.spectrum,
            marklab::AnalysisSection::InsufficientData { reason }
                if reason.contains("degenerate") && reason.contains("mark-homogeneous")
        ),
        "spectrum={:?}",
        result.spectrum
    );
    assert_eq!(
        result.primary_endpoint.null,
        "stratified_fixed_position_random_labeling"
    );
    assert_eq!(
        result
            .mark_pair_covariance
            .value()
            .and_then(|summary| summary.p_global),
        Some(1.0),
        "mark-pair-covariance inference must use the configured stratified null"
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
            .multiscale_residual
            .value()
            .and_then(|summary| summary.block_mean_variance_fraction_p_value.value())
            .copied(),
        Some(1.0),
        "multiscale_residual inference must use the configured stratified null"
    );
    assert!(
        result
            .status_flags
            .contains(&StatusFlag::DegenerateSpatialStrataNull),
        "flags={:?}, spectral_curve_test={:?}",
        result.status_flags,
        result
            .spectrum
            .value()
            .map(|value| &value.spectral_curve_test)
    );
    assert!(!result
        .status_flags
        .contains(&StatusFlag::ConfoundedBySpatialStrata));
    let sensitivity = result
        .spectrum_null_sensitivity
        .value()
        .expect("degenerate sensitivity summary");
    assert_eq!(
        sensitivity.conclusion,
        SpectrumConfoundingConclusion::DegenerateStratifiedNull
    );
    assert!(sensitivity.unstratified.value().is_some());
    assert!(matches!(
        &sensitivity.stratified,
        AnalysisSection::InsufficientData { reason }
            if reason.contains("degenerate") && reason.contains("mark-homogeneous")
    ));
    assert_eq!(result.status, "suppressed");
}

#[test]
fn distinct_nulls_are_actually_executed() {
    let mut pattern = Pattern::from_arrays(
        (0..40).map(|index| index as f64).collect(),
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
            .map(|index| if index < 9 { 10_u16 } else { 20_u16 })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );

    let mut unstratified_config = permissive_config();
    unstratified_config.permutation.b = 199;
    unstratified_config.inference.family_wise_alpha = 0.10;
    let unstratified = AnalysisEngine::new(unstratified_config)
        .expect("unstratified engine")
        .analyze_pattern(&pattern)
        .expect("unstratified analysis");

    let mut stratified_config = permissive_config();
    stratified_config.permutation.b = 199;
    stratified_config.inference.family_wise_alpha = 0.10;
    stratified_config.permutation.stratified = true;
    stratified_config.permutation.strata_fields = vec![PermutationStratum::QcBin];
    let stratified = AnalysisEngine::new(stratified_config)
        .expect("stratified engine")
        .analyze_pattern(&pattern)
        .expect("stratified analysis");

    let unstratified_p = unstratified
        .spectrum
        .value()
        .and_then(|summary| summary.low_k_excess_p_value)
        .expect("unstratified p-value");
    let stratified_p = stratified
        .spectrum
        .value()
        .and_then(|summary| summary.low_k_excess_p_value)
        .expect("stratified p-value");
    assert!(unstratified_p < 0.10, "unstratified p={unstratified_p}");
    assert!(stratified_p >= 0.10, "stratified p={stratified_p}");
    assert_eq!(
        stratified.primary_endpoint.null,
        "stratified_fixed_position_random_labeling"
    );
    let sensitivity = stratified
        .spectrum_null_sensitivity
        .value()
        .expect("stratified spectrum sensitivity summary");
    assert_eq!(
        sensitivity.primary_null,
        SpectrumNullModel::StratifiedFixedPositionRandomLabeling
    );
    assert_eq!(
        sensitivity.conclusion,
        SpectrumConfoundingConclusion::ConfoundedBySpatialStrata
    );
    let reported_unstratified = sensitivity
        .unstratified
        .value()
        .expect("unstratified sensitivity inference");
    let reported_stratified = sensitivity
        .stratified
        .value()
        .expect("stratified primary inference");
    assert_eq!(
        reported_unstratified.low_k_excess_p_value,
        Some(unstratified_p)
    );
    assert_eq!(reported_stratified.low_k_excess_p_value, Some(stratified_p));
    assert!(matches!(
        unstratified.spectrum_null_sensitivity,
        AnalysisSection::NotApplicable
    ));

    let document = ResultDocument::marked(stratified.clone());
    let json = serde_json::to_string(&document).expect("serialize sensitivity result");
    let roundtrip = ResultDocument::from_json(&json)
        .expect("deserialize sensitivity result")
        .into_marked_pattern()
        .expect("marked result");
    assert_eq!(
        roundtrip.spectrum_null_sensitivity,
        stratified.spectrum_null_sensitivity
    );
    assert!(
        stratified
            .status_flags
            .contains(&StatusFlag::ConfoundedBySpatialStrata),
        "distinct nulls imply confounding, but flags were {:?}",
        stratified.status_flags
    );
}

#[test]
fn missing_strata_report_validation_error() {
    let mut config = permissive_config();
    config.permutation.stratified = true;
    config.permutation.strata_fields = vec![PermutationStratum::QcBin];
    let mut pattern = Pattern::from_arrays(
        (0..40).map(|index| index as f64).collect(),
        vec![0.0; 40],
        (0..40).map(|index| u8::from(index < 8)).collect(),
        meta(),
    )
    .expect("pattern");
    pattern.window.l_eff_um = 40.0;
    pattern.window.d_nn_mean_um = 1.0;
    pattern.window.area_um2 = 40.0;

    let error = AnalysisEngine::new(config)
        .expect("engine")
        .analyze_pattern(&pattern)
        .expect_err("missing configured strata must fail validation");

    assert!(error
        .to_string()
        .contains("configured permutation stratum QcBin is absent"));
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
            .contains(&StatusFlag::DegenerateSpatialStrataNull),
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
    assert_eq!(
        result.component_mode_selection.selected,
        ResolvedComponentMode::Both
    );
    assert!(!result.component_mode_selection.reason.is_empty());
    assert!(result.spectrum.value().is_some());
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
    assert!(matches!(
        pooled.component_results,
        marklab::AnalysisSection::NotApplicable
    ));
    assert_eq!(
        pooled.component_mode_selection.selected,
        ResolvedComponentMode::Pooled
    );
    assert!(pooled.spectrum.value().is_some());

    let mut auto_config = permissive_config();
    auto_config.analysis.analyze_components = ComponentMode::Auto;
    auto_config.validation.n_min = 6;
    auto_config.validation.n_marked_min = 1;
    auto_config.validation.n_unmarked_min = 1;
    auto_config.validation.k_shell_min = 1;
    auto_config.spectrum.k_shells = 8;
    auto_config.permutation.b = 19;
    auto_config.inference.family_wise_alpha = 0.25;
    let auto_both = AnalysisEngine::new(auto_config.clone())
        .expect("auto engine")
        .analyze_pattern(&pattern)
        .expect("auto analysis");
    assert_eq!(
        auto_both.component_mode_selection.selected,
        ResolvedComponentMode::Both
    );
    assert!(auto_both
        .component_results
        .value()
        .is_some_and(|components| components.len() == 2));
    assert!(auto_both.component_mode_selection.reason.contains("0.500"));

    let mut no_components = pattern.clone();
    no_components.component_id = None;
    let auto_pooled = AnalysisEngine::new(auto_config)
        .expect("auto engine")
        .analyze_pattern(&no_components)
        .expect("auto pooled analysis");
    assert_eq!(
        auto_pooled.component_mode_selection.selected,
        ResolvedComponentMode::Pooled
    );
    assert!(matches!(
        auto_pooled.component_results,
        marklab::AnalysisSection::NotApplicable
    ));
    assert!(auto_pooled
        .component_mode_selection
        .reason
        .contains("unavailable"));
}

#[test]
fn remediation_separate_component_mode_does_not_behave_like_both() {
    let mut config = permissive_config();
    config.analysis.analyze_components = ComponentMode::Separate;
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

    assert!(result
        .component_results
        .value()
        .is_some_and(|components| components.len() == 2));
    assert!(
        matches!(result.spectrum, marklab::AnalysisSection::NotApplicable),
        "Separate must not expose the pooled spectrum as the active analysis: {:?}",
        result.spectrum
    );
    assert_eq!(
        result.component_mode_selection.selected,
        ResolvedComponentMode::Separate
    );
    assert!(!result.component_mode_selection.reason.is_empty());
    assert!(matches!(
        result.primary_endpoint.value,
        marklab::AnalysisSection::NotApplicable
    ));
    assert!(matches!(
        result.primary_endpoint.p_value,
        marklab::AnalysisSection::NotApplicable
    ));
    assert!(result.spectrum_curve.is_empty());
    assert!(matches!(
        result.mark_pair_covariance,
        marklab::AnalysisSection::NotApplicable
    ));
    assert!(matches!(
        result.anisotropy,
        marklab::AnalysisSection::NotApplicable
    ));
    assert!(matches!(
        result.multiscale_residual,
        marklab::AnalysisSection::NotApplicable
    ));
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
    config.multiscale_residual.enabled = true;
    config.multiscale_residual.territory_detection = true;
    config.multiscale_residual.min_territory_z = 2.0;

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
        .residual_territories
        .value()
        .expect("multiscale residual territories");
    assert!(territories.len() >= 2, "{territories:?}");
    assert!(
        *result
            .multiscale_residual
            .value()
            .expect("multiscale residual")
            .block_mean_variance_fraction_p_value
            .value()
            .expect("block-mean variance p-value")
            > 0.0
    );
    assert!(
        *result
            .multiscale_residual
            .value()
            .expect("multiscale residual")
            .territory_count_p_value
            .value()
            .expect("territory count p-value")
            > 0.0
    );
    assert!(result
        .residual_territories
        .value()
        .expect("multiscale residual territories")
        .iter()
        .any(|territory| territory.center_x_um < 3.0 && territory.center_y_um < 3.0));
    assert!(result
        .residual_territories
        .value()
        .expect("multiscale residual territories")
        .iter()
        .any(|territory| territory.center_x_um > 6.0 && territory.center_y_um > 6.0));
    assert!(territories.iter().all(|territory| {
        territory.residual_score.is_finite()
            && territory.analysis_scale_um.is_finite()
            && territory.supporting_marked_cells > 0
            && territory.qc_overlap_fraction.is_none()
    }));
}
