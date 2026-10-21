#![forbid(unsafe_code)]
//! Causal-design research workflows for Marklab.

#[doc(hidden)]
pub mod protocol;

mod eig;
mod exposure_mapping;
mod interference;
mod sensitivity;

pub use sensitivity::{
    binary_confounder_bias_sensitivity, manski_bounded_outcome_ate, rosenbaum_sign_sensitivity,
    BiasSensitivityResult, BiasSensitivityScenario, BiasSensitivityScenarioResult,
    BiasSensitivitySpec, BoundedOutcomeObservation, ManskiBoundedOutcomeResult,
    ManskiBoundedOutcomeSpec, MatchedPairObservation, MatchedPairSet, RosenbaumSensitivityPoint,
    RosenbaumSignSensitivityResult, RosenbaumSignSensitivitySpec,
};

pub use eig::{
    estimate_gaussian_expected_information_gain, GaussianEigOuterValue, GaussianEigResult,
    GaussianEigSpec,
};

pub use exposure_mapping::{
    compute_spatial_exposure_mapping, ExposureGraphEdge, ExposureMappingKind,
    ExposureMappingResult, ExposureMappingSpec, ExposureMappingUnit, ExposureValueStatus,
    UnitExposureValue,
};

pub use interference::{
    randomized_binary_interference, BaselineCovariate, CausalError, CausalUnit, ClusterAssignment,
    ExposureContrast, ExposureContrastStatus, ExposureMeanEstimate, ExposureMeanStatus,
    InterferenceEdge, InterferenceRandomizationResult, JointBinaryExposure,
    JointExposureProbabilities, ObservedUnitExposure, RandomizedInterferenceResult,
    RandomizedInterferenceSpec, UnitExposureProbabilities,
};
