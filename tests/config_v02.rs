use marklab::{
    AnalysisConfig, NeighborhoodNullModel, PermutationStratum, RegistrationTransform, ThreadSetting,
};

#[test]
fn default_config_matches_the_supported_v02_controls() {
    let config = AnalysisConfig::default();

    assert_eq!(config.analysis.mark_label, "marked");
    assert_eq!(config.inference.family_wise_alpha, 0.05);
    assert!(!config.diagnostics.beta_binomial);
    assert!(!config.diagnostics.graph_smoothing);
    assert_eq!(config.registration.transform, RegistrationTransform::Affine);
    assert_eq!(config.performance.threads, ThreadSetting::Auto);
    assert_eq!(
        config.permutation.strata_fields,
        vec![PermutationStratum::QcBin, PermutationStratum::ComponentId]
    );
    assert_eq!(
        config.neighborhood.null_models,
        vec![
            NeighborhoodNullModel::SourceSection,
            NeighborhoodNullModel::SourceSectionDensity,
            NeighborhoodNullModel::SourceSectionCellClass,
            NeighborhoodNullModel::SourceSectionRegistrationQc,
        ]
    );
}

#[test]
fn config_rejects_removed_keys_without_aliases() {
    for old_key in [
        "[analysis]\nphenotype_name = \"legacy\"",
        "[analysis]\nprimary_endpoint = \"low_k_excess\"",
        "[permutation]\nmode = \"fixed_position_random_labeling\"",
        "[permutation]\nglobal_envelope = \"erl\"",
        "[performance]\ndeterministic_parallel = true",
        "[output]\nformat_version = \"0.1\"",
        "[comparison]\nequivalence_required_for_same = true",
        "[modeling]\nbayesian = true",
    ] {
        let error = AnalysisConfig::from_toml_overrides(old_key)
            .expect_err("removed configuration key must be rejected");
        assert!(
            error
                .to_string()
                .contains(old_key.lines().nth(1).unwrap().split(' ').next().unwrap())
                || error.to_string().contains("modeling"),
            "unexpected error for {old_key:?}: {error}"
        );
    }
}

#[test]
fn config_errors_identify_the_nested_field_path() {
    let error = AnalysisConfig::from_toml_overrides(
        "[validation]\nlargest_interpretable_scale_fraction = \"wide\"",
    )
    .expect_err("type mismatch");

    assert!(
        error
            .to_string()
            .contains("validation.largest_interpretable_scale_fraction"),
        "unexpected error: {error}"
    );
}

#[test]
fn config_validates_statistical_resolution_and_cross_fields() {
    let underpowered = AnalysisConfig::from_toml_overrides(
        "[permutation]\nb = 19\n[inference]\nfamily_wise_alpha = 0.05",
    )
    .expect_err("two-sided endpoints need sufficient resolution");
    assert!(underpowered.to_string().contains("B + 1 >= 2 / alpha"));

    let invalid_range =
        AnalysisConfig::from_toml_overrides("[validation]\np_min = 0.8\np_max = 0.2")
            .expect_err("invalid prevalence interval");
    assert!(invalid_range.to_string().contains("validation.p_min"));

    let unsupported_combination =
        AnalysisConfig::from_toml_overrides("[analysis]\nuse_probabilistic_marks = true")
            .expect_err("probabilistic marks do not yet have a stratified null");
    assert!(unsupported_combination
        .to_string()
        .contains("analysis.use_probabilistic_marks"));
}

#[test]
fn config_parses_only_supported_typed_values() {
    let config = AnalysisConfig::from_toml_overrides(
        r#"
[registration]
transform = "rigid"

[permutation]
strata_fields = ["component_id"]

[neighborhood]
null_models = ["source_section_density"]

[performance]
threads = 4
"#,
    )
    .expect("supported typed values");

    assert_eq!(config.registration.transform, RegistrationTransform::Rigid);
    assert_eq!(
        config.permutation.strata_fields,
        vec![PermutationStratum::ComponentId]
    );
    assert_eq!(
        config.neighborhood.null_models,
        vec![NeighborhoodNullModel::SourceSectionDensity]
    );
    assert_eq!(config.performance.threads, ThreadSetting::Count(4));

    let invalid = AnalysisConfig::from_toml_overrides("[registration]\ntransform = \"projective\"")
        .expect_err("unsupported transform");
    assert!(invalid.to_string().contains("registration.transform"));
}

#[test]
fn config_uses_multiscale_residual_terms_without_obsolete_aliases() {
    let config = AnalysisConfig::from_toml_overrides(
        "[multiscale_residual]\nenabled = false\nterritory_detection = false\nmin_territory_z = 3.0",
    )
    .expect("accurately named multiscale residual section");
    let value = toml::Value::try_from(&config).expect("serialize config");

    assert!(value.get("multiscale_residual").is_some());
    assert!(value.get("wavelet").is_none());
    assert!(AnalysisConfig::from_toml_overrides("[wavelet]\nenabled = false").is_err());
}

#[test]
fn config_uses_mark_pair_covariance_margin_without_obsolete_alias() {
    let config =
        AnalysisConfig::from_toml_overrides("[comparison.margins]\nmark_pair_covariance = 0.25")
            .expect("accurately named mark-pair covariance margin");

    assert_eq!(config.comparison.margins.mark_pair_covariance, Some(0.25));
    assert!(
        AnalysisConfig::from_toml_overrides("[comparison.margins]\npair_correlation = 0.25")
            .is_err()
    );
    assert!(AnalysisConfig::from_toml_overrides(
        "[comparison.equivalence_margins]\nmark_pair_covariance = 0.25"
    )
    .is_err());

    let exact = AnalysisConfig::from_toml_overrides("[comparison.margins]\nspectrum = 0.0")
        .expect("zero is a valid exact-match descriptive margin");
    assert_eq!(exact.comparison.margins.spectrum, Some(0.0));
}
