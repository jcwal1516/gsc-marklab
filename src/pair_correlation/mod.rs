mod analysis;
mod types;

pub(crate) use analysis::configuration_digest;
pub use analysis::homogeneous_pair_correlation;
pub use types::{
    HomogeneousPairCorrelationConfig, HomogeneousPairCorrelationError,
    HomogeneousPairCorrelationInference, HomogeneousPairCorrelationResult, PairCorrelationKernel,
    PairCorrelationPoint, PairCorrelationPointStatus,
};

pub(crate) fn epanechnikov_weight(
    radius_um: f64,
    distance_um: f64,
    bandwidth_um: f64,
) -> Option<f64> {
    let scaled = (radius_um - distance_um) / bandwidth_um;
    (scaled.abs() < 1.0).then(|| 0.75 * (1.0 - scaled * scaled) / bandwidth_um)
}
