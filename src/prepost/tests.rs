use crate::{
    comparison::{
        margin_assessment::curve_margin_assessment,
        pooled_bin_difference::pooled_bin_difference_diagnostic,
    },
    prepost::deltas::compare_prepost,
    AnalysisSection, AnisotropySummary, ComponentMode, ComponentModeSelection,
    CrossInteractionCurve, CrossInteractionPoint, CurveComparisonAvailability, FunctionalSummary,
    Interpretation, MarkPairCovariancePoint, MarkedPatternResult, MultiscaleResidualSummary,
    PrimaryEndpoint, QcSummary, ResidualTerritory, ResolvedComponentMode, ScaleEnergyPoint,
    SpectrumPoint, SpectrumSummary, WindowSummary,
};

fn minimal_analysis_result(case_id: &str, timepoint: &str) -> MarkedPatternResult {
    MarkedPatternResult {
        case_id: case_id.into(),
        timepoint: timepoint.into(),
        protein: "MSH6".into(),
        mark_label: "marked".into(),
        status: "ok".into(),
        status_flags: Vec::new(),
        n_cells: 4,
        n_marked: 2,
        p_hat: 0.5,
        window: WindowSummary {
            area_um2: 100.0,
            l_eff_um: 10.0,
            d_nn_mean_um: 1.0,
        },
        qc: QcSummary::default(),
        primary_endpoint: PrimaryEndpoint {
            name: "low_k_excess".into(),
            value: AnalysisSection::available(1.0),
            p_value: AnalysisSection::available(1.0),
            null: "fixed_position_random_labeling".into(),
        },
        spectrum: AnalysisSection::available(SpectrumSummary {
            max_interpretable_scale_um: 10.0,
            k_min: Some(1.0),
            k_max: Some(2.0),
            n_k_modes: 2,
            n_shells: 2,
            n_permutations: 19,
            spectral_curve_test: AnalysisSection::available(FunctionalSummary {
                p_global: Some(1.0),
                erl_depth: Some(1.0),
                n_permutations: 19,
            }),
            xi_um: Some(10.0),
            xi_stability_interval_um: Some([8.0, 12.0]),
            low_k_excess: 1.0,
            low_k_excess_p_value: Some(1.0),
            alpha: Some(0.0),
            xi_um_p_value: Some(1.0),
            alpha_p_value: Some(1.0),
        }),
        spectrum_curve: Vec::new(),
        mark_pair_covariance: AnalysisSection::available(FunctionalSummary::default()),
        mark_pair_covariance_curve: Vec::new(),
        anisotropy: AnalysisSection::available(AnisotropySummary {
            index: 1.0,
            theta_deg: None,
            p_value: Some(1.0),
        }),
        multiscale_residual: AnalysisSection::available(MultiscaleResidualSummary {
            local_difference_energy_fraction: 0.0,
            residual_energy_fraction: 0.0,
            block_mean_variance_fraction: 0.0,
            block_mean_to_local_difference_ratio: None,
            territory_count: 0,
            block_mean_variance_fraction_p_value: AnalysisSection::available(1.0),
            territory_count_p_value: AnalysisSection::available(1.0),
        }),
        scale_energy: AnalysisSection::available(FunctionalSummary::default()),
        scale_energy_curve: Vec::<ScaleEnergyPoint>::new(),
        residual_territories: AnalysisSection::available(Vec::new()),
        component_mode_selection: ComponentModeSelection {
            requested: ComponentMode::Pooled,
            selected: ResolvedComponentMode::Pooled,
            reason: "test fixture".into(),
        },
        component_results: AnalysisSection::available(Vec::new()),
        diagnostics: AnalysisSection::Disabled,
        timings: Vec::new(),
        interpretation: Interpretation {
            class: "random_like".into(),
            text: "No unsafe biological mechanism claim.".into(),
        },
        registration: AnalysisSection::NotApplicable,
        fused_cell_summary: AnalysisSection::NotApplicable,
        fused_cells: Vec::new(),
        neighborhood_enrichment: AnalysisSection::NotApplicable,
        cross_interaction_curves: AnalysisSection::NotApplicable,
        territory_profiles: AnalysisSection::NotApplicable,
        territory_comparisons: AnalysisSection::NotApplicable,
        prepost_curve_comparisons: Vec::new(),
    }
}

fn territory(center_x_um: f64) -> ResidualTerritory {
    ResidualTerritory {
        center_x_um,
        center_y_um: 0.0,
        radius_um: 10.0,
        analysis_scale_um: 7.0,
        residual_score: 2.0,
        supporting_marked_cells: 2,
        component_id: None,
        qc_overlap_fraction: None,
    }
}

#[test]
fn pooled_bin_difference_and_margin_assessment_have_distinct_interpretations() {
    let pre = [1.0, 1.1, 0.9];
    let post = [1.01, 1.09, 0.91];

    let difference =
        pooled_bin_difference_diagnostic("spectrum", &pre, &post, 19, 123).expect("difference");
    let margin_assessment =
        curve_margin_assessment("spectrum", &pre, &post, Some(0.2)).expect("margin assessment");

    assert!(difference.pooled_bin_p_value.is_some());
    assert_eq!(margin_assessment.within_margin, Some(true));
    assert!(!difference.interpretation.contains("same"));
}

#[test]
fn curve_margin_assessment_has_no_equivalence_p_value() {
    let result = curve_margin_assessment("spectrum", &[1.0, 2.0], &[1.0, 2.1], Some(0.2))
        .expect("margin assessment");
    let value = serde_json::to_value(result).expect("comparison json");

    assert_eq!(value["margin"], 0.2);
    assert_eq!(value["within_margin"], true);
    assert!(value.get("equivalence_margin").is_none());
    assert!(value.get("p_equivalence").is_none());
    assert!(value.get("equivalent").is_none());
}

#[test]
fn pooled_bin_difference_uses_diagnostic_schema() {
    let result = pooled_bin_difference_diagnostic("spectrum", &[1.0, 2.0], &[1.0, 2.5], 19, 123)
        .expect("pooled-bin diagnostic");
    let value = serde_json::to_value(result).expect("comparison json");

    assert!(value["pooled_bin_p_value"].is_number());
    assert_eq!(value["method"], "pooled_bin_permutation");
    assert!(value.get("p_difference").is_none());
}

#[test]
fn prepost_result_includes_curve_comparisons_when_curves_exist() {
    let mut pre = minimal_analysis_result("case1", "pre");
    let mut post = minimal_analysis_result("case1", "post");

    pre.spectrum_curve = vec![
        SpectrumPoint {
            k: 1.0,
            observed_power: 1.0,
            median_permutation_power: 1.0,
            whitened_power: 1.0,
            inference_eligible: true,
            lower_global_envelope: Some(0.8),
            upper_global_envelope: Some(1.2),
        },
        SpectrumPoint {
            k: 2.0,
            observed_power: 1.1,
            median_permutation_power: 1.0,
            whitened_power: 1.1,
            inference_eligible: true,
            lower_global_envelope: Some(0.8),
            upper_global_envelope: Some(1.2),
        },
    ];
    post.spectrum_curve = pre.spectrum_curve.clone();
    post.spectrum_curve[1].whitened_power = 1.12;

    pre.mark_pair_covariance_curve = vec![MarkPairCovariancePoint {
        r_min_um: 0.0,
        r_max_um: 10.0,
        covariance: Some(0.1),
        inference_eligible: true,
        lower_global_envelope: Some(0.0),
        upper_global_envelope: Some(0.2),
        pair_count: 10,
    }];
    pre.mark_pair_covariance
        .value_mut()
        .expect("mark-pair covariance")
        .n_permutations = 7;
    post.mark_pair_covariance_curve = vec![MarkPairCovariancePoint {
        r_min_um: 0.0,
        r_max_um: 10.0,
        covariance: Some(0.11),
        inference_eligible: true,
        lower_global_envelope: Some(0.0),
        upper_global_envelope: Some(0.2),
        pair_count: 10,
    }];
    post.mark_pair_covariance
        .value_mut()
        .expect("mark-pair covariance")
        .n_permutations = 7;

    let delta = compare_prepost(&pre, &post);
    assert!(!delta.curve_comparisons.is_empty());
    for comparison_name in ["spectrum", "mark_pair_covariance"] {
        let comparison_tests: Vec<_> = delta
            .curve_comparisons
            .iter()
            .filter(|test| test.comparison_name == comparison_name)
            .collect();
        assert_eq!(
            comparison_tests.len(),
            2,
            "{comparison_name} should emit difference-diagnostic and margin-assessment rows"
        );
        assert!(comparison_tests
            .iter()
            .any(|test| test.pooled_bin_p_value.is_some()));
        assert!(comparison_tests.iter().any(|test| {
            test.pooled_bin_p_value.is_none()
                && test.margin.is_none()
                && test.within_margin.is_none()
                && test
                    .interpretation
                    .contains("unavailable without a prespecified descriptive margin")
        }));
    }
}

#[test]
fn prepost_result_includes_multimodal_cross_interaction_tests_and_territory_delta() {
    let mut pre = minimal_analysis_result("case1", "pre");
    let mut post = minimal_analysis_result("case1", "post");

    pre.cross_interaction_curves = AnalysisSection::available(vec![CrossInteractionCurve {
        label_a: "mmr_abnormal".into(),
        label_b: "lymphocyte".into(),
        points: vec![
            CrossInteractionPoint {
                r_min_um: 0.0,
                r_max_um: 10.0,
                value: Some(2.0),
                inference_eligible: true,
                lower_global_envelope: Some(0.0),
                upper_global_envelope: Some(3.0),
                count: 2,
            },
            CrossInteractionPoint {
                r_min_um: 10.0,
                r_max_um: 20.0,
                value: Some(1.0),
                inference_eligible: true,
                lower_global_envelope: Some(0.0),
                upper_global_envelope: Some(3.0),
                count: 1,
            },
        ],
        p_global: Some(0.5),
    }]);
    post.cross_interaction_curves = AnalysisSection::available(vec![CrossInteractionCurve {
        label_a: "mmr_abnormal".into(),
        label_b: "lymphocyte".into(),
        points: vec![
            CrossInteractionPoint {
                r_min_um: 0.0,
                r_max_um: 10.0,
                value: Some(3.0),
                inference_eligible: true,
                lower_global_envelope: Some(0.0),
                upper_global_envelope: Some(4.0),
                count: 3,
            },
            CrossInteractionPoint {
                r_min_um: 10.0,
                r_max_um: 20.0,
                value: Some(1.0),
                inference_eligible: true,
                lower_global_envelope: Some(0.0),
                upper_global_envelope: Some(3.0),
                count: 1,
            },
        ],
        p_global: Some(0.25),
    }]);
    pre.residual_territories = AnalysisSection::available(vec![territory(0.0)]);
    post.residual_territories = AnalysisSection::available(vec![territory(0.0), territory(50.0)]);

    let delta = compare_prepost(&pre, &post);

    assert_eq!(delta.delta_territory_count.value(), Some(&1));
    let cross_tests: Vec<_> = delta
        .curve_comparisons
        .iter()
        .filter(|test| test.comparison_name == "cross_interaction:mmr_abnormal/lymphocyte")
        .collect();
    assert_eq!(cross_tests.len(), 2);
    assert!(cross_tests
        .iter()
        .any(|test| test.pooled_bin_p_value.is_some()));
}

#[test]
fn prepost_curve_comparisons_surface_absent_curves_as_diagnostics() {
    let pre = minimal_analysis_result("case1", "pre");
    let post = minimal_analysis_result("case1", "post");

    let delta = compare_prepost(&pre, &post);

    for comparison_name in ["spectrum", "mark_pair_covariance"] {
        let comparison_tests: Vec<_> = delta
            .curve_comparisons
            .iter()
            .filter(|test| test.comparison_name == comparison_name)
            .collect();
        assert_eq!(
            comparison_tests.len(),
            1,
            "{comparison_name} should emit one absent-curve diagnostic"
        );
        let diagnostic = comparison_tests[0];
        assert_eq!(
            diagnostic.availability,
            CurveComparisonAvailability::InsufficientData
        );
        assert_eq!(diagnostic.statistic, None);
        assert!(diagnostic.unavailable_reason.is_some());
        assert_eq!(diagnostic.metric, "curve_availability");
        assert!(diagnostic.pooled_bin_p_value.is_none());
        assert!(diagnostic.margin.is_none());
        assert!(diagnostic.within_margin.is_none());
        let encoded = serde_json::to_value(diagnostic).expect("diagnostic JSON");
        assert_eq!(encoded["availability"], "insufficient_data");
        assert!(encoded["statistic"].is_null());
        assert!(encoded["unavailable_reason"].is_string());
        assert!(diagnostic.interpretation.contains("absent"));
    }
}

#[test]
fn mark_pair_covariance_difference_uses_mark_pair_covariance_permutation_count() {
    let mut pre = minimal_analysis_result("case1", "pre");
    let mut post = minimal_analysis_result("case1", "post");

    pre.spectrum.value_mut().expect("spectrum").n_permutations = 19;
    post.spectrum.value_mut().expect("spectrum").n_permutations = 19;
    pre.mark_pair_covariance
        .value_mut()
        .expect("mark-pair covariance")
        .n_permutations = 0;
    post.mark_pair_covariance
        .value_mut()
        .expect("mark-pair covariance")
        .n_permutations = 0;

    pre.mark_pair_covariance_curve = vec![MarkPairCovariancePoint {
        r_min_um: 0.0,
        r_max_um: 10.0,
        covariance: Some(0.1),
        inference_eligible: true,
        lower_global_envelope: Some(0.0),
        upper_global_envelope: Some(0.2),
        pair_count: 10,
    }];
    post.mark_pair_covariance_curve = vec![MarkPairCovariancePoint {
        r_min_um: 0.0,
        r_max_um: 10.0,
        covariance: Some(0.2),
        inference_eligible: true,
        lower_global_envelope: Some(0.0),
        upper_global_envelope: Some(0.2),
        pair_count: 10,
    }];

    let delta = compare_prepost(&pre, &post);
    let pair_tests: Vec<_> = delta
        .curve_comparisons
        .iter()
        .filter(|test| test.comparison_name == "mark_pair_covariance")
        .collect();

    assert_eq!(pair_tests.len(), 2);
    assert!(pair_tests.iter().any(|test| {
        test.availability == CurveComparisonAvailability::InsufficientData
            && test.statistic.is_none()
            && test.pooled_bin_p_value.is_none()
            && test
                .interpretation
                .contains("permutations must be greater than zero")
    }));
    assert!(pair_tests.iter().any(|test| {
        test.pooled_bin_p_value.is_none()
            && test.margin.is_none()
            && test.within_margin.is_none()
            && test
                .interpretation
                .contains("unavailable without a prespecified descriptive margin")
    }));
}

#[test]
fn prepost_curve_comparisons_surface_unaligned_axis_diagnostics() {
    let mut pre = minimal_analysis_result("case1", "pre");
    let mut post = minimal_analysis_result("case1", "post");

    pre.spectrum_curve = vec![
        SpectrumPoint {
            k: 1.0,
            observed_power: 1.0,
            median_permutation_power: 1.0,
            whitened_power: 1.0,
            inference_eligible: true,
            lower_global_envelope: Some(0.8),
            upper_global_envelope: Some(1.2),
        },
        SpectrumPoint {
            k: 2.0,
            observed_power: 1.1,
            median_permutation_power: 1.0,
            whitened_power: 1.1,
            inference_eligible: true,
            lower_global_envelope: Some(0.8),
            upper_global_envelope: Some(1.2),
        },
    ];
    post.spectrum_curve = vec![
        SpectrumPoint {
            k: 1.0,
            observed_power: 1.0,
            median_permutation_power: 1.0,
            whitened_power: 1.0,
            inference_eligible: true,
            lower_global_envelope: Some(0.8),
            upper_global_envelope: Some(1.2),
        },
        SpectrumPoint {
            k: 3.0,
            observed_power: 1.1,
            median_permutation_power: 1.0,
            whitened_power: 1.1,
            inference_eligible: true,
            lower_global_envelope: Some(0.8),
            upper_global_envelope: Some(1.2),
        },
    ];

    pre.mark_pair_covariance_curve = vec![MarkPairCovariancePoint {
        r_min_um: 0.0,
        r_max_um: 10.0,
        covariance: Some(0.1),
        inference_eligible: true,
        lower_global_envelope: Some(0.0),
        upper_global_envelope: Some(0.2),
        pair_count: 10,
    }];
    post.mark_pair_covariance_curve = vec![MarkPairCovariancePoint {
        r_min_um: 5.0,
        r_max_um: 15.0,
        covariance: Some(0.11),
        inference_eligible: true,
        lower_global_envelope: Some(0.0),
        upper_global_envelope: Some(0.2),
        pair_count: 10,
    }];

    let delta = compare_prepost(&pre, &post);

    for comparison_name in ["spectrum", "mark_pair_covariance"] {
        let comparison_tests: Vec<_> = delta
            .curve_comparisons
            .iter()
            .filter(|test| test.comparison_name == comparison_name)
            .collect();
        assert_eq!(
            comparison_tests.len(),
            1,
            "{comparison_name} should emit one unaligned-axis diagnostic"
        );
        let diagnostic = comparison_tests[0];
        assert_eq!(
            diagnostic.availability,
            CurveComparisonAvailability::InsufficientData
        );
        assert_eq!(diagnostic.statistic, None);
        assert!(diagnostic.pooled_bin_p_value.is_none());
        assert!(diagnostic.margin.is_none());
        assert!(diagnostic.within_margin.is_none());
        assert!(diagnostic.interpretation.contains("axis"));
    }
}

#[test]
fn remediation_prepost_axes_accept_harmless_float_reconstruction() {
    let mut pre = minimal_analysis_result("case1", "pre");
    let mut post = minimal_analysis_result("case1", "post");
    pre.spectrum_curve = vec![SpectrumPoint {
        k: 0.1 + 0.2,
        observed_power: 1.0,
        median_permutation_power: 1.0,
        whitened_power: 1.0,
        inference_eligible: true,
        lower_global_envelope: Some(0.8),
        upper_global_envelope: Some(1.2),
    }];
    post.spectrum_curve = vec![SpectrumPoint {
        k: 0.3,
        observed_power: 1.0,
        median_permutation_power: 1.0,
        whitened_power: 1.01,
        inference_eligible: true,
        lower_global_envelope: Some(0.8),
        upper_global_envelope: Some(1.2),
    }];
    pre.mark_pair_covariance
        .value_mut()
        .expect("mark-pair covariance")
        .n_permutations = 19;
    post.mark_pair_covariance
        .value_mut()
        .expect("mark-pair covariance")
        .n_permutations = 19;
    pre.mark_pair_covariance_curve = vec![MarkPairCovariancePoint {
        r_min_um: 0.1 + 0.2,
        r_max_um: 0.2 + 0.2,
        covariance: Some(0.1),
        inference_eligible: true,
        lower_global_envelope: Some(0.0),
        upper_global_envelope: Some(0.2),
        pair_count: 1,
    }];
    post.mark_pair_covariance_curve = vec![MarkPairCovariancePoint {
        r_min_um: 0.3,
        r_max_um: 0.4,
        covariance: Some(0.11),
        ..pre.mark_pair_covariance_curve[0].clone()
    }];
    pre.cross_interaction_curves = AnalysisSection::available(vec![CrossInteractionCurve {
        label_a: "mmr_abnormal".into(),
        label_b: "lymphocyte".into(),
        points: vec![CrossInteractionPoint {
            r_min_um: 0.1 + 0.2,
            r_max_um: 0.2 + 0.2,
            value: Some(2.0),
            inference_eligible: true,
            lower_global_envelope: Some(1.0),
            upper_global_envelope: Some(3.0),
            count: 2,
        }],
        p_global: Some(0.5),
    }]);
    post.cross_interaction_curves = AnalysisSection::available(vec![CrossInteractionCurve {
        label_a: "mmr_abnormal".into(),
        label_b: "lymphocyte".into(),
        points: vec![CrossInteractionPoint {
            r_min_um: 0.3,
            r_max_um: 0.4,
            value: Some(2.1),
            ..pre
                .cross_interaction_curves
                .value()
                .expect("cross interaction")[0]
                .points[0]
                .clone()
        }],
        p_global: Some(0.5),
    }]);

    let delta = compare_prepost(&pre, &post);
    for comparison_name in [
        "spectrum",
        "mark_pair_covariance",
        "cross_interaction:mmr_abnormal/lymphocyte",
    ] {
        let comparison_tests = delta
            .curve_comparisons
            .iter()
            .filter(|test| test.comparison_name == comparison_name)
            .collect::<Vec<_>>();

        assert_eq!(
            comparison_tests.len(),
            2,
            "matching reconstructed axes should run difference and margin diagnostics: {comparison_tests:?}"
        );
        assert!(comparison_tests
            .iter()
            .all(|test| !test.interpretation.contains("axis")));
    }
}

#[test]
fn prepost_axes_reject_material_numeric_differences() {
    let mut pre = minimal_analysis_result("case1", "pre");
    let mut post = minimal_analysis_result("case1", "post");
    pre.spectrum_curve = vec![SpectrumPoint {
        k: 0.3,
        observed_power: 1.0,
        median_permutation_power: 1.0,
        whitened_power: 1.0,
        inference_eligible: true,
        lower_global_envelope: Some(0.8),
        upper_global_envelope: Some(1.2),
    }];
    post.spectrum_curve = vec![SpectrumPoint {
        k: 0.3 + 1e-8,
        whitened_power: 1.01,
        ..pre.spectrum_curve[0].clone()
    }];

    let delta = compare_prepost(&pre, &post);
    let spectrum_tests = delta
        .curve_comparisons
        .iter()
        .filter(|test| test.comparison_name == "spectrum")
        .collect::<Vec<_>>();

    assert_eq!(spectrum_tests.len(), 1);
    assert_eq!(
        spectrum_tests[0].availability,
        CurveComparisonAvailability::InsufficientData
    );
    assert!(spectrum_tests[0]
        .unavailable_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("axis differs")));
}
