use serde::Serialize;

use crate::{AgreementStatus, SensitivityStatus};
use crate::{ClaimStatus, FitState, McseScalarSummary};

/// Agreement of one posterior mean between the PyMC and NumPyro backends.
#[derive(Debug, Serialize)]
pub struct ParameterAgreement {
    pub pymc_mean: f64,
    pub numpyro_mean: f64,
    pub absolute_difference: f64,
    pub combined_mcse: f64,
    pub standardized_difference: f64,
    pub tolerance: f64,
    pub intervals_overlap: bool,
    pub passes: bool,
}

/// Agreement of paired element-wise posterior means, such as a latent field's nodes.
#[derive(Debug, Serialize)]
pub struct ElementwiseAgreement {
    pub root_mean_square_difference: f64,
    pub maximum_absolute_difference: f64,
    pub maximum_standardized_difference: f64,
    pub all_intervals_overlap: bool,
    pub passes: bool,
}

/// Agreement of per-patient posterior means.
#[derive(Debug, Serialize)]
pub struct PatientAgreement {
    pub patient_count: usize,
    #[serde(flatten)]
    pub agreement: ElementwiseAgreement,
}

/// `difference / mcse`; `f64::MAX` stands in for an infinite ratio so results stay finite.
pub(crate) fn standardized_mcse_difference(difference: f64, mcse: f64) -> f64 {
    if mcse > 0.0 {
        difference / mcse
    } else if difference == 0.0 {
        0.0
    } else {
        f64::MAX
    }
}

/// Standard error of the difference between two independent backends' posterior means.
pub fn combined_mcse(left: &McseScalarSummary, right: &McseScalarSummary) -> f64 {
    left.mcse_mean.hypot(right.mcse_mean)
}

pub fn intervals_overlap(left: &McseScalarSummary, right: &McseScalarSummary) -> bool {
    left.interval_lower <= right.interval_upper && right.interval_lower <= left.interval_upper
}

/// Compares one posterior mean across backends. The means agree when their intervals overlap
/// and they differ by at most `max(minimum_tolerance, maximum_standardized_difference *
/// combined_mcse)`, where `combined_mcse` combines the two quantities' own MCSEs.
pub fn compare_parameter(
    pymc: &McseScalarSummary,
    numpyro: &McseScalarSummary,
    maximum_standardized_difference: f64,
    minimum_tolerance: f64,
) -> ParameterAgreement {
    let absolute_difference = (pymc.mean - numpyro.mean).abs();
    let combined_mcse = combined_mcse(pymc, numpyro);
    let standardized_difference = standardized_mcse_difference(absolute_difference, combined_mcse);
    let tolerance = minimum_tolerance.max(maximum_standardized_difference * combined_mcse);
    let intervals_overlap = intervals_overlap(pymc, numpyro);
    ParameterAgreement {
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

/// Applies [`compare_parameter`] to every `(pymc, numpyro)` pair; all pairs must pass.
pub fn compare_elements(
    pairs: impl IntoIterator<Item = (McseScalarSummary, McseScalarSummary)>,
    maximum_standardized_difference: f64,
    minimum_tolerance: f64,
) -> ElementwiseAgreement {
    let mut count = 0_usize;
    let mut squared = 0.0;
    let mut maximum_absolute = 0.0_f64;
    let mut maximum_standardized = 0.0_f64;
    let mut overlaps = true;
    let mut passes = true;
    for (pymc, numpyro) in pairs {
        let element = compare_parameter(
            &pymc,
            &numpyro,
            maximum_standardized_difference,
            minimum_tolerance,
        );
        count += 1;
        squared += element.absolute_difference * element.absolute_difference;
        maximum_absolute = maximum_absolute.max(element.absolute_difference);
        maximum_standardized = maximum_standardized.max(element.standardized_difference);
        overlaps &= element.intervals_overlap;
        passes &= element.passes;
    }
    ElementwiseAgreement {
        root_mean_square_difference: (squared / count as f64).sqrt(),
        maximum_absolute_difference: maximum_absolute,
        maximum_standardized_difference: maximum_standardized,
        all_intervals_overlap: overlaps,
        passes,
    }
}

/// Fit state, agreement status and claim of a two-backend comparison. The comparison is
/// available only when both fits are complete and every compared quantity agrees.
pub(crate) struct AgreementVerdict {
    pub(crate) fit_state: FitState,
    pub(crate) agreement_status: AgreementStatus,
    pub(crate) claim_status: ClaimStatus,
}

impl AgreementVerdict {
    pub(crate) fn new(pymc: FitState, numpyro: FitState, agrees: bool) -> Self {
        let fits_complete = pymc == FitState::Complete && numpyro == FitState::Complete;
        let available = fits_complete && agrees;
        Self {
            fit_state: if fits_complete {
                FitState::Complete
            } else {
                FitState::Nonconverged
            },
            agreement_status: if available {
                AgreementStatus::AgreeWithinMonteCarloError
            } else if fits_complete {
                AgreementStatus::DiagnosticOnlyBackendDisagreement
            } else {
                AgreementStatus::DiagnosticOnlyNonconverged
            },
            claim_status: if available {
                ClaimStatus::ExperimentalCrossBackendValidation
            } else {
                ClaimStatus::DiagnosticOnlyCrossBackendValidation
            },
        }
    }
}

/// The existing result vocabulary differs for hierarchy, kernel, and tail sensitivity.
pub(crate) enum SensitivityKind {
    Prior,
    HierarchicalPrior,
    Kernel,
    Tail,
}

/// Shared completion and scientific-claim gate for a declared sensitivity grid.
pub(crate) struct SensitivityVerdict {
    pub(crate) fit_state: FitState,
    pub(crate) status: SensitivityStatus,
    pub(crate) claim_status: ClaimStatus,
}

impl SensitivityVerdict {
    pub(crate) fn new(all_complete: bool, any_material_shift: bool, kind: SensitivityKind) -> Self {
        let status = match (all_complete, any_material_shift, &kind) {
            (false, _, _) => SensitivityStatus::DiagnosticOnlyNonconverged,
            (true, true, SensitivityKind::HierarchicalPrior) => {
                SensitivityStatus::SensitiveWithinDeclaredPriorGrid
            }
            (true, false, SensitivityKind::HierarchicalPrior) => {
                SensitivityStatus::StableWithinDeclaredPriorGrid
            }
            (true, true, _) => SensitivityStatus::SensitiveWithinDeclaredGrid,
            (true, false, _) => SensitivityStatus::StableWithinDeclaredGrid,
        };
        let claim_status = match (all_complete, kind) {
            (true, SensitivityKind::Prior | SensitivityKind::HierarchicalPrior) => {
                ClaimStatus::ExperimentalPriorSensitivity
            }
            (false, SensitivityKind::Prior | SensitivityKind::HierarchicalPrior) => {
                ClaimStatus::DiagnosticOnlyPriorSensitivity
            }
            (true, SensitivityKind::Kernel) => ClaimStatus::ExperimentalPriorKernelSensitivity,
            (false, SensitivityKind::Kernel) => ClaimStatus::DiagnosticOnlyPriorKernelSensitivity,
            (true, SensitivityKind::Tail) => ClaimStatus::ExperimentalPriorTailSensitivity,
            (false, SensitivityKind::Tail) => ClaimStatus::DiagnosticOnlyPriorTailSensitivity,
        };
        Self {
            fit_state: if all_complete {
                FitState::Complete
            } else {
                FitState::Nonconverged
            },
            status,
            claim_status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(mean: f64, sd: f64, mcse_mean: f64) -> McseScalarSummary {
        McseScalarSummary {
            mean,
            sd,
            interval_lower: mean - 2.0 * sd,
            interval_upper: mean + 2.0 * sd,
            mcse_mean,
        }
    }

    #[test]
    fn tolerance_uses_each_quantitys_own_monte_carlo_error() {
        // A well-mixed quantity (MCSE 0.001) must not inherit the tolerance that the worst
        // parameter's effective sample size would imply (sd / sqrt(ESS) = 0.1 / sqrt(4) = 0.05).
        let pymc = summary(1.0, 0.1, 0.001);
        let numpyro = summary(1.02, 0.1, 0.001);
        let agreement = compare_parameter(&pymc, &numpyro, 4.0, 1e-3);
        assert!((agreement.combined_mcse - 0.001_f64.hypot(0.001)).abs() < 1e-15);
        assert!((agreement.tolerance - 4.0 * agreement.combined_mcse).abs() < 1e-15);
        assert!(agreement.intervals_overlap);
        assert!(!agreement.passes);

        let noisy = compare_parameter(
            &summary(1.0, 0.1, 0.01),
            &summary(1.02, 0.1, 0.01),
            4.0,
            1e-3,
        );
        assert!(noisy.passes);
    }

    #[test]
    fn minimum_tolerance_and_interval_overlap_bound_the_verdict() {
        let floor = compare_parameter(
            &summary(1.0, 0.1, 1e-6),
            &summary(1.0005, 0.1, 1e-6),
            4.0,
            1e-3,
        );
        assert_eq!(floor.tolerance, 1e-3);
        assert!(floor.passes);

        let disjoint = compare_parameter(
            &summary(0.0, 0.01, 1.0),
            &summary(1.0, 0.01, 1.0),
            4.0,
            1e-3,
        );
        assert!(!disjoint.intervals_overlap);
        assert!(!disjoint.passes);
    }

    #[test]
    fn elements_pass_only_when_every_pair_passes() {
        let pairs = [
            (summary(0.2, 0.05, 0.002), summary(0.201, 0.05, 0.002)),
            (summary(0.4, 0.05, 0.002), summary(0.43, 0.05, 0.002)),
        ];
        let agreement = compare_elements(pairs, 4.0, 1e-3);
        assert!((agreement.maximum_absolute_difference - 0.03).abs() < 1e-12);
        assert!(agreement.all_intervals_overlap);
        assert!(!agreement.passes);
        assert!(compare_elements(pairs.into_iter().take(1), 4.0, 1e-3).passes);
    }
}
