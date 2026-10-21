//! The [`SensitivityStatus`] vocabulary.

marklab_core::vocabulary_enum! {
    /// Whether any scenario in a declared sensitivity grid moved the result materially.
    pub enum SensitivityStatus {
        DecisionSensitive => "decision_sensitive",
        DecisionStable => "decision_stable",
        DiagnosticOnlyNonconverged => "diagnostic_only_nonconverged",
        MaterialPriorSensitivity => "material_prior_sensitivity",
        SensitiveWithinDeclaredGrid => "sensitive_within_declared_grid",
        SensitiveWithinDeclaredPriorGrid => "sensitive_within_declared_prior_grid",
        StableBelowDeclaredThreshold => "stable_below_declared_threshold",
        StableWithinDeclaredGrid => "stable_within_declared_grid",
        StableWithinDeclaredPriorGrid => "stable_within_declared_prior_grid",
    }
}
