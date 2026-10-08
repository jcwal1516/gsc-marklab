use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct ResourceSegment {
    pub resource_id: String,
    pub start_x_um: f64,
    pub start_y_um: f64,
    pub end_x_um: f64,
    pub end_y_um: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DistanceOutcomeObservation {
    pub cell_id: String,
    pub patient_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub outcome: f64,
    pub compartment: String,
    pub resource_density: f64,
    pub accessibility: f64,
}

#[derive(Clone, Debug)]
pub struct DistanceToResourceSpec {
    pub resources: Vec<ResourceSegment>,
    pub spline_knots_um: Vec<f64>,
    pub observations: Vec<DistanceOutcomeObservation>,
    pub coefficient_prior_sd: f64,
    pub patient_effect_prior_sd: f64,
    pub known_noise_sd: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DistanceToResourceModel {
    pub family: &'static str,
    pub distance_basis: &'static str,
    pub distance_semantics: &'static str,
    pub hierarchy: [&'static str; 1],
    pub patient_effect: &'static str,
    pub likelihood: &'static str,
    pub coefficient_prior_sd: f64,
    pub patient_effect_prior_sd: f64,
    pub known_noise_sd: f64,
    pub spline_knots_um: Vec<f64>,
    pub reference_compartment: String,
    pub inference_method: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct DerivedResourceDistance {
    pub cell_id: String,
    pub nearest_resource_id: String,
    pub unsigned_distance_um: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GaussianPosteriorSummary {
    pub name: String,
    pub mean: f64,
    pub sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DistancePosterior {
    pub fixed_coefficients: Vec<GaussianPosteriorSummary>,
    pub patient_effects: Vec<GaussianPosteriorSummary>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DistanceResponsePoint {
    pub distance_um: f64,
    pub posterior_mean: f64,
    pub posterior_sd: f64,
    pub interval_lower: f64,
    pub interval_upper: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DistancePrediction {
    pub cell_id: String,
    pub patient_id: String,
    pub observed: f64,
    pub posterior_predictive_mean: f64,
    pub posterior_predictive_sd: f64,
    pub residual: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PatientPredictiveCheck {
    pub patient_id: String,
    pub observations: u32,
    pub observed_mean: f64,
    pub posterior_predictive_mean: f64,
    pub mean_residual: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResourcePredictiveCheck {
    pub resource_id: String,
    pub observations: u32,
    pub observed_mean: f64,
    pub posterior_predictive_mean: f64,
    pub mean_residual: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DistancePosteriorPredictive {
    pub rmse: f64,
    pub mean_residual: f64,
    pub patient_checks: Vec<PatientPredictiveCheck>,
    pub resource_checks: Vec<ResourcePredictiveCheck>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DistanceToResourceFit {
    pub format: &'static str,
    pub version: u32,
    pub model: DistanceToResourceModel,
    pub resources: Vec<ResourceSegment>,
    pub distances: Vec<DerivedResourceDistance>,
    pub posterior: DistancePosterior,
    pub distance_response: Vec<DistanceResponsePoint>,
    pub predictions: Vec<DistancePrediction>,
    pub posterior_predictive: DistancePosteriorPredictive,
    pub claim_status: &'static str,
}
