//! The [`ModelMaturity`] vocabulary.

crate::vocabulary_enum! {
    /// How far a model's implementation and validation have progressed.
    pub enum ModelMaturity {
        Experimental => "experimental",
        ExperimentalApproximation => "experimental_approximation",
        ExperimentalCalibration => "experimental_calibration",
        ExperimentalMinimumContrast => "experimental_minimum_contrast",
        ExperimentalModelConstruction => "experimental_model_construction",
        ExperimentalPseudolikelihood => "experimental_pseudolikelihood",
        ExperimentalSensitivity => "experimental_sensitivity",
    }
}
