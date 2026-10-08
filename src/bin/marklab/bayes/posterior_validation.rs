use marklab_bayes::{DiagnosticPolicy, NormalMeanDiagnostics, SarScalarSummary};

pub(crate) fn summary_valid(summary: &SarScalarSummary) -> bool {
    [
        summary.mean,
        summary.sd,
        summary.interval_lower,
        summary.interval_upper,
    ]
    .into_iter()
    .all(f64::is_finite)
        && summary.sd >= 0.0
        && summary.interval_lower <= summary.interval_upper
}

pub(crate) fn scalar_valid(summary: &SarScalarSummary, positive: bool) -> bool {
    summary_valid(summary) && (!positive || (summary.mean > 0.0 && summary.interval_lower >= 0.0))
}

pub(crate) fn diagnostics_pass(
    diagnostics: &NormalMeanDiagnostics,
    policy: &DiagnosticPolicy,
) -> bool {
    diagnostics.prior_predictive_finite
        && diagnostics.posterior_finite
        && diagnostics.constraints_valid
        && diagnostics.identifiability_checks_passed
        && diagnostics.r_hat <= policy.maximum_r_hat
        && diagnostics.ess_bulk >= policy.minimum_bulk_ess
        && diagnostics.ess_tail >= policy.minimum_tail_ess
        && diagnostics.minimum_ebfmi >= policy.minimum_ebfmi
        && diagnostics.divergences <= policy.maximum_divergences
        && diagnostics.max_tree_depth_hits <= policy.maximum_tree_depth_hits
}
