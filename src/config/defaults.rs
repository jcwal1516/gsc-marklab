use super::model::{
    AnalysisConfig, AnalysisConfigSection, ComparisonSection, ComponentMode, DiagnosticsSection,
    InferenceSection, MultiscaleResidualSection, NeighborhoodNullModel, NeighborhoodSection,
    OutputSection, PerformanceSection, PeriodogramSection, PermutationSection, PermutationStratum,
    RegistrationSection, RegistrationTransform, SpectrumSection, ThreadSetting, ValidationSection,
};

impl Default for RegistrationSection {
    fn default() -> Self {
        Self {
            enabled: true,
            transform: RegistrationTransform::Affine,
            min_landmarks: 6,
            max_rmse_um: 25.0,
            claim_distance_multiplier: 2.0,
        }
    }
}

impl Default for NeighborhoodSection {
    fn default() -> Self {
        Self {
            enabled: true,
            radius_um: 50.0,
            k_nearest: 8,
            label_pairs: vec![
                ["mmr_abnormal".into(), "mmr_abnormal".into()],
                ["mmr_abnormal".into(), "lymphocyte".into()],
            ],
            territory_eps_um: 50.0,
            territory_min_cells: 1,
            territory_min_radius_um: 1.0,
            null_models: vec![
                NeighborhoodNullModel::SourceSection,
                NeighborhoodNullModel::SourceSectionDensity,
                NeighborhoodNullModel::SourceSectionCellClass,
                NeighborhoodNullModel::SourceSectionRegistrationQc,
            ],
        }
    }
}

impl Default for OutputSection {
    fn default() -> Self {
        Self {
            write_parquet_curves: true,
            write_geojson_territories: true,
            write_figures: true,
            write_run_manifest: true,
        }
    }
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            analysis: AnalysisConfigSection {
                mark_label: "marked".into(),
                use_probabilistic_marks: false,
                analyze_components: ComponentMode::Auto,
            },
            validation: ValidationSection {
                n_min: 200,
                n_marked_min: 25,
                n_unmarked_min: 25,
                p_min: 0.02,
                p_max: 0.98,
                area_min_um2: 100_000.0,
                k_shell_min: 5,
                largest_interpretable_scale_fraction: 0.33,
                valid_mask_fraction_min: 0.5,
            },
            spectrum: SpectrumSection {
                k_shells: 64,
                low_k_shells: 3,
                fit_low_k_alpha: true,
                anisotropy_low_k_shells: 5,
            },
            periodogram: PeriodogramSection { enabled: true },
            multiscale_residual: MultiscaleResidualSection {
                enabled: true,
                territory_detection: true,
                min_territory_z: 2.5,
            },
            permutation: PermutationSection {
                b: 999,
                seed: 123_456_789,
                stratified: true,
                strata_fields: vec![PermutationStratum::QcBin, PermutationStratum::ComponentId],
            },
            inference: InferenceSection {
                family_wise_alpha: 0.05,
            },
            diagnostics: DiagnosticsSection::default(),
            registration: RegistrationSection::default(),
            neighborhood: NeighborhoodSection::default(),
            comparison: ComparisonSection::default(),
            performance: PerformanceSection {
                threads: ThreadSetting::Auto,
                memory_budget_mib: 4096,
                k_chunk_modes: 256,
                strict_repro: false,
                save_intermediates: false,
            },
            output: OutputSection::default(),
        }
    }
}
