use crate::{DiagnosticPolicy, McseScalarSummary, PosteriorDiagnostics};

/// A finite summary with an ordered interval, non-negative SD and non-negative MCSE.
pub fn summary_valid(summary: &McseScalarSummary) -> bool {
    [
        summary.mean,
        summary.sd,
        summary.interval_lower,
        summary.interval_upper,
        summary.mcse_mean,
    ]
    .into_iter()
    .all(f64::is_finite)
        && summary.sd >= 0.0
        && summary.mcse_mean >= 0.0
        && summary.interval_lower <= summary.interval_upper
}

pub fn scalar_valid(summary: &McseScalarSummary, positive: bool) -> bool {
    summary_valid(summary) && (!positive || (summary.mean > 0.0 && summary.interval_lower >= 0.0))
}

pub fn diagnostics_pass(diagnostics: &PosteriorDiagnostics, policy: &DiagnosticPolicy) -> bool {
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
