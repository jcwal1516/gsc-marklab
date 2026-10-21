//! The [`StatisticalUnit`] vocabulary.

crate::vocabulary_enum! {
    /// The unit a result's statistics describe, and so the population it can speak about.
    pub enum StatisticalUnit {
        One3dPointPatternObservedInOneSegmentedVolume => "one_3d_point_pattern_observed_in_one_segmented_volume",
        OneFixedLocationPattern => "one_fixed_location_pattern",
        OneObservedPointPattern => "one_observed_point_pattern",
        OnePatientLesionObservedAtTwoBiopsyTimepoints => "one_patient_lesion_observed_at_two_biopsy_timepoints",
        OneRegisteredSpecimenVolumeAtOnePatientTimepoint => "one_registered_specimen_volume_at_one_patient_timepoint",
        OneSpecimenGraph => "one_specimen_graph",
        OneSpecimenPointPattern => "one_specimen_point_pattern",
        Patient => "patient",
        PriorGenerativePointPattern => "prior_generative_point_pattern",
        RegionConditionalOnDeclaredGraph => "region_conditional_on_declared_graph",
        WithinSpecimenFieldDiagnostic => "within_specimen_field_diagnostic",
    }
}
