use std::{fs, path::PathBuf};

use marklab_bayes::{
    fit_distance_to_resource_model, DistanceOutcomeObservation, DistanceToResourceSpec,
    ResourceSegment,
};
use serde::Deserialize;

use super::{publish_json, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    resources: Vec<ResourceInput>,
    spline_knots_um: Vec<f64>,
    observations: Vec<ObservationInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceInput {
    resource_id: String,
    start_x_um: f64,
    start_y_um: f64,
    end_x_um: f64,
    end_y_um: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationInput {
    cell_id: String,
    patient_id: String,
    x_um: f64,
    y_um: f64,
    outcome: f64,
    compartment: String,
    resource_density: f64,
    accessibility: f64,
}

pub(super) fn run(
    input_path: PathBuf,
    coefficient_prior_sd: f64,
    patient_effect_prior_sd: f64,
    known_noise_sd: f64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let metadata = fs::metadata(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "distance-to-resource input must be a regular file within 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input_path).map_err(|source| BayesCliError::Io {
        path: input_path,
        source,
    })?;
    let input: Input = serde_json::from_slice(&bytes)?;
    let result = fit_distance_to_resource_model(DistanceToResourceSpec {
        resources: input
            .resources
            .into_iter()
            .map(|resource| ResourceSegment {
                resource_id: resource.resource_id,
                start_x_um: resource.start_x_um,
                start_y_um: resource.start_y_um,
                end_x_um: resource.end_x_um,
                end_y_um: resource.end_y_um,
            })
            .collect(),
        spline_knots_um: input.spline_knots_um,
        observations: input
            .observations
            .into_iter()
            .map(|row| DistanceOutcomeObservation {
                cell_id: row.cell_id,
                patient_id: row.patient_id,
                x_um: row.x_um,
                y_um: row.y_um,
                outcome: row.outcome,
                compartment: row.compartment,
                resource_density: row.resource_density,
                accessibility: row.accessibility,
            })
            .collect(),
        coefficient_prior_sd,
        patient_effect_prior_sd,
        known_noise_sd,
    })?;
    publish_json(&output_path, &result)
}
