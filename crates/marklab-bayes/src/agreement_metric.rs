use crate::{BetaBinomialParameterAgreement, SarScalarSummary};

pub(crate) fn standardized_mcse_difference(difference: f64, mcse: f64) -> f64 {
    if mcse > 0.0 {
        difference / mcse
    } else if difference == 0.0 {
        0.0
    } else {
        f64::INFINITY
    }
}

pub(crate) fn compare_beta_binomial_parameter(
    pymc: &SarScalarSummary,
    pymc_ess: f64,
    numpyro: &SarScalarSummary,
    numpyro_ess: f64,
    maximum_standardized_difference: f64,
    minimum_tolerance: f64,
) -> BetaBinomialParameterAgreement {
    let absolute_difference = (pymc.mean - numpyro.mean).abs();
    let combined_mcse = (pymc.sd / pymc_ess.sqrt()).hypot(numpyro.sd / numpyro_ess.sqrt());
    let standardized_difference = standardized_mcse_difference(absolute_difference, combined_mcse);
    let tolerance = minimum_tolerance.max(maximum_standardized_difference * combined_mcse);
    let intervals_overlap = pymc.interval_lower <= numpyro.interval_upper
        && numpyro.interval_lower <= pymc.interval_upper;
    BetaBinomialParameterAgreement {
        pymc_mean: pymc.mean,
        numpyro_mean: numpyro.mean,
        absolute_difference,
        combined_mcse,
        standardized_difference,
        tolerance,
        intervals_overlap,
        passes: intervals_overlap && absolute_difference <= tolerance,
    }
}
