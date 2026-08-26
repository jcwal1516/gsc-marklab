#![forbid(unsafe_code)]
//! Machine-readable execution and maturity policies for Marklab.

use serde::{Deserialize, Serialize};
use thiserror::Error;

mod execution_mode;
mod runtime_validation;
mod validation_ladder;

pub use runtime_validation::{
    deterministic_parallel_reduce, run_runtime_validation, BayesianHmcFitDiagnosticSpec,
    BenchmarkResult, CalibrationResult, CalibrationScenario, DiagnosticCheck, FitDiagnostics,
    ParallelReductionEvidence, RuntimeValidationResult, RuntimeValidationSpec,
};

pub use validation_ladder::{
    evaluate_validation_ladder, RealDataValidationLadderResult, ValidationLadderSpec,
    ValidationStageRecord,
};

pub use execution_mode::{
    select_execution_mode, ExecutionModeAssessment, ExecutionModeDescriptor,
    ExecutionModeSelection, ExecutionModeSelectionSpec, ExecutionModeSelectionStatus,
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Maturity {
    Validated,
    Established,
    Experimental,
    ResearchOnly,
    UnsupportedForClaim,
}

impl Maturity {
    const fn rank(self) -> u8 {
        match self {
            Self::UnsupportedForClaim => 0,
            Self::ResearchOnly => 1,
            Self::Experimental => 2,
            Self::Established => 3,
            Self::Validated => 4,
        }
    }

    fn cap(self, maximum: Self) -> Self {
        if self.rank() <= maximum.rank() {
            self
        } else {
            maximum
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionModeKind {
    Exact,
    Approximate,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultMaturitySpec {
    pub method_maturity: Maturity,
    pub mode: ExecutionModeKind,
    pub provenance_complete: bool,
    pub converged: bool,
    pub severe_diagnostic_failure: bool,
    pub approximation_error_validated: bool,
    pub predictive_clinical_claim: bool,
    pub external_validation_complete: bool,
    pub causal_claim: bool,
    pub causal_identification_supported: bool,
    pub bayesian: bool,
    pub sbc_complete: bool,
    pub posterior_predictive_complete: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResultMaturityDecision {
    pub format: &'static str,
    pub version: u32,
    pub method_maturity: Maturity,
    pub result_maturity: Maturity,
    pub downgraded: bool,
    pub reasons: Vec<&'static str>,
    pub policy: &'static str,
}

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("invalid policy input: {0}")]
    Invalid(String),
}

pub fn determine_result_maturity(
    spec: ResultMaturitySpec,
) -> Result<ResultMaturityDecision, PolicyError> {
    validate_maturity_spec(&spec)?;
    let mut maturity = spec.method_maturity;
    let mut reasons = Vec::new();
    if !spec.provenance_complete {
        maturity = Maturity::UnsupportedForClaim;
        reasons.push("provenance_incomplete");
    }
    if !spec.converged {
        maturity = Maturity::UnsupportedForClaim;
        reasons.push("nonconverged");
    }
    if spec.severe_diagnostic_failure {
        maturity = Maturity::UnsupportedForClaim;
        reasons.push("severe_diagnostic_failure");
    }
    if spec.mode == ExecutionModeKind::Approximate && !spec.approximation_error_validated {
        maturity = maturity.cap(Maturity::ResearchOnly);
        reasons.push("approximate_mode_without_validated_error_or_calibration");
    }
    if spec.predictive_clinical_claim && !spec.external_validation_complete {
        maturity = maturity.cap(Maturity::Experimental);
        reasons.push("predictive_clinical_claim_without_external_validation");
    }
    if spec.causal_claim && !spec.causal_identification_supported {
        maturity = Maturity::UnsupportedForClaim;
        reasons.push("causal_identification_unsupported");
    }
    if spec.bayesian && (!spec.sbc_complete || !spec.posterior_predictive_complete) {
        maturity = maturity.cap(Maturity::Experimental);
        reasons.push("bayesian_requirements_incomplete");
    }
    Ok(ResultMaturityDecision {
        format: "marklab.result_maturity_decision",
        version: 1,
        method_maturity: spec.method_maturity,
        result_maturity: maturity,
        downgraded: maturity != spec.method_maturity,
        reasons,
        policy: "part_xiii_result_maturity_v1",
    })
}

fn validate_maturity_spec(spec: &ResultMaturitySpec) -> Result<(), PolicyError> {
    if spec.method_maturity == Maturity::UnsupportedForClaim
        && (spec.predictive_clinical_claim || spec.causal_claim)
    {
        return Err(PolicyError::Invalid(
            "unsupported methods cannot request predictive-clinical or causal claim evaluation"
                .into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incomplete_provenance_is_terminal() {
        let decision = determine_result_maturity(ResultMaturitySpec {
            method_maturity: Maturity::Established,
            mode: ExecutionModeKind::Exact,
            provenance_complete: false,
            converged: true,
            severe_diagnostic_failure: false,
            approximation_error_validated: true,
            predictive_clinical_claim: false,
            external_validation_complete: false,
            causal_claim: false,
            causal_identification_supported: false,
            bayesian: false,
            sbc_complete: false,
            posterior_predictive_complete: false,
        })
        .unwrap();
        assert_eq!(decision.result_maturity, Maturity::UnsupportedForClaim);
        assert_eq!(decision.reasons, vec!["provenance_incomplete"]);
    }
}
