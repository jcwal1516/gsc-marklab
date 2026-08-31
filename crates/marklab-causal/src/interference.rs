mod estimation;
mod model;
mod validation;
mod workflow;

pub use model::{
    BaselineCovariate, CausalError, CausalUnit, ClusterAssignment, ExposureContrast,
    ExposureMeanEstimate, InterferenceEdge, InterferenceRandomizationResult, JointBinaryExposure,
    JointExposureProbabilities, ObservedUnitExposure, RandomizedInterferenceResult,
    RandomizedInterferenceSpec, UnitExposureProbabilities,
};
pub use workflow::randomized_binary_interference;

const MAXIMUM_ASSIGNMENT_STATES: u64 = 1_000_000;
const MAXIMUM_PERMUTATIONS: usize = 1_000_000;

#[cfg(test)]
mod tests;
