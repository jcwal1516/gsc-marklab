//! The [`AgreementStatus`] vocabulary.

marklab_core::vocabulary_enum! {
    /// Whether two backends' posterior summaries agree within Monte Carlo error.
    pub enum AgreementStatus {
        AgreeWithinMonteCarloError => "agree_within_monte_carlo_error",
        DiagnosticOnlyBackendDisagreement => "diagnostic_only_backend_disagreement",
        DiagnosticOnlyDisagreementOrNonconvergence => "diagnostic_only_disagreement_or_nonconvergence",
        DiagnosticOnlyNonconverged => "diagnostic_only_nonconverged",
    }
}
