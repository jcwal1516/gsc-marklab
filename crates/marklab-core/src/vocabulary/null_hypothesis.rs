//! The [`NullHypothesis`] vocabulary.

crate::vocabulary_enum! {
    /// The null hypothesis a fitted model or test is compared against.
    pub enum NullHypothesis {
        AllTypeSpecificComparisonGroupLogIntensityEffectsEqualZero => "all_type_specific_comparison_group_log_intensity_effects_equal_zero",
        ComparisonGroupLogIntensityEffectEqualsZero => "comparison_group_log_intensity_effect_equals_zero",
        EveryOffDiagonalLatentFieldCorrelationEqualsZero => "every_off_diagonal_latent_field_correlation_equals_zero",
        EveryTypeComparisonGroupLogIntensityEffectEqualsZero => "every_type_comparison_group_log_intensity_effect_equals_zero",
        FittedLogLinearInhomogeneousPoissonProcessConditionalOnFixedCovariate => "fitted_log_linear_inhomogeneous_poisson_process_conditional_on_fixed_covariate",
        FixedLocationIndependentCategoricalLabelsInterceptOnly => "fixed_location_independent_categorical_labels_intercept_only",
        HomogeneousProcessConditionedOnExactVoxelUnionObservationWindow => "homogeneous_process_conditioned_on_exact_voxel_union_observation_window",
        LabelsPermutedWithinDeclaredStrata => "labels_permuted_within_declared_strata",
        NoneDescriptiveLatentIntensityModel => "none_descriptive_latent_intensity_model",
        ObservedKChangeIsExplainableBySuppliedRegistrationDeformationDraws => "observed_k_change_is_explainable_by_supplied_registration_deformation_draws",
        PriorGenerativeOrDeclaredStressScenarioUnderTheSameQuadratureLikelihood => "prior_generative_or_declared_stress_scenario_under_the_same_quadrature_likelihood",
        ZeroPatientPopulationGroupShiftInFixedLocationConditionalMarkAffinity => "zero_patient_population_group_shift_in_fixed_location_conditional_mark_affinity",
        ZeroSharedPatientEcologyLoadingAndZeroGroupEffects => "zero_shared_patient_ecology_loading_and_zero_group_effects",
        ZeroSpatialFactorEmbeddingIncrementOverNonspatialPatientModel => "zero_spatial_factor_embedding_increment_over_nonspatial_patient_model",
    }
}
