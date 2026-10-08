use std::collections::{BTreeMap, BTreeSet};

use crate::BayesError;

mod linear_gaussian;
mod types;

use linear_gaussian::{linear_summary, posterior};
pub use types::*;

pub fn fit_distance_to_resource_model(
    mut spec: DistanceToResourceSpec,
) -> Result<DistanceToResourceFit, BayesError> {
    validate_and_sort(&mut spec)?;
    let patients = spec
        .observations
        .iter()
        .map(|row| row.patient_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let compartments = spec
        .observations
        .iter()
        .map(|row| row.compartment.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let distances = derive_distances(&spec);
    let fixed_names = fixed_names(&spec.spline_knots_um, &compartments);
    let dimension = fixed_names.len() + patients.len();
    if dimension > 128 || spec.observations.len() <= dimension {
        return Err(BayesError::InvalidSpec(
            "distance model requires more observations than at most 128 coefficients".into(),
        ));
    }
    let patient_index = patients
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut design = Vec::with_capacity(spec.observations.len() * dimension);
    for (row, distance) in spec.observations.iter().zip(&distances) {
        design.extend(features(
            row,
            distance.unsigned_distance_um,
            &spec.spline_knots_um,
            &compartments,
            &patient_index,
            fixed_names.len(),
        ));
    }
    let (posterior_mean, posterior_covariance) =
        posterior(&spec, &design, dimension, fixed_names.len())?;
    let fixed_coefficients = fixed_names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            summary(
                name.clone(),
                posterior_mean[index],
                posterior_covariance[index * dimension + index],
            )
        })
        .collect();
    let patient_effects = patients
        .iter()
        .enumerate()
        .map(|(offset, patient)| {
            let index = fixed_names.len() + offset;
            summary(
                patient.clone(),
                posterior_mean[index],
                posterior_covariance[index * dimension + index],
            )
        })
        .collect();
    let predictions = predictions(
        &spec,
        &design,
        dimension,
        &posterior_mean,
        &posterior_covariance,
    );
    let posterior_predictive = predictive_checks(&predictions, &distances);
    let distance_response = response_curve(
        &spec,
        &distances,
        &compartments,
        dimension,
        &posterior_mean,
        &posterior_covariance,
    );
    Ok(DistanceToResourceFit {
        format: "marklab.distance_to_resource_fit",
        version: 1,
        model: DistanceToResourceModel {
            family: "gaussian_hierarchical_resource_distance_spline",
            distance_basis: "linear_hinge_spline",
            distance_semantics: "unsigned_euclidean_point_to_segment",
            hierarchy: ["patient"],
            patient_effect: "zero_mean_normal_random_intercept_fixed_prior_scale",
            likelihood: "normal_known_sigma",
            coefficient_prior_sd: spec.coefficient_prior_sd,
            patient_effect_prior_sd: spec.patient_effect_prior_sd,
            known_noise_sd: spec.known_noise_sd,
            spline_knots_um: spec.spline_knots_um,
            reference_compartment: compartments[0].clone(),
            inference_method: "exact_conjugate_multivariate_normal",
        },
        resources: spec.resources,
        distances,
        posterior: DistancePosterior {
            fixed_coefficients,
            patient_effects,
        },
        distance_response,
        predictions,
        posterior_predictive,
        claim_status: "association_not_transport_or_resource_causality",
    })
}

fn validate_and_sort(spec: &mut DistanceToResourceSpec) -> Result<(), BayesError> {
    if !(1..=1_000).contains(&spec.resources.len())
        || !(8..=5_000).contains(&spec.observations.len())
        || spec.spline_knots_um.len() > 16
    {
        return Err(BayesError::InvalidSpec(
            "distance model resource, observation, or knot count is invalid".into(),
        ));
    }
    for value in [
        spec.coefficient_prior_sd,
        spec.patient_effect_prior_sd,
        spec.known_noise_sd,
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(BayesError::InvalidSpec(
                "distance model prior and noise scales must be finite and positive".into(),
            ));
        }
    }
    spec.resources
        .sort_by(|left, right| left.resource_id.cmp(&right.resource_id));
    for (index, resource) in spec.resources.iter().enumerate() {
        let coordinates = [
            resource.start_x_um,
            resource.start_y_um,
            resource.end_x_um,
            resource.end_y_um,
        ];
        if invalid_id(&resource.resource_id)
            || coordinates.iter().any(|value| !value.is_finite())
            || (resource.start_x_um == resource.end_x_um
                && resource.start_y_um == resource.end_y_um)
            || (index > 0 && spec.resources[index - 1].resource_id == resource.resource_id)
        {
            return Err(BayesError::InvalidSpec(
                "resource segments require unique IDs and finite nonzero geometry".into(),
            ));
        }
    }
    spec.spline_knots_um.sort_by(f64::total_cmp);
    if spec
        .spline_knots_um
        .iter()
        .enumerate()
        .any(|(index, knot)| {
            !knot.is_finite()
                || *knot <= 0.0
                || (index > 0 && spec.spline_knots_um[index - 1] == *knot)
        })
    {
        return Err(BayesError::InvalidSpec(
            "distance spline knots must be unique finite positive values".into(),
        ));
    }
    spec.observations.sort_by(|left, right| {
        left.patient_id
            .cmp(&right.patient_id)
            .then_with(|| left.cell_id.cmp(&right.cell_id))
    });
    let mut patient_counts = BTreeMap::<String, usize>::new();
    let mut cell_ids = BTreeSet::new();
    for row in &spec.observations {
        if invalid_id(&row.cell_id)
            || invalid_id(&row.patient_id)
            || invalid_id(&row.compartment)
            || !cell_ids.insert(row.cell_id.clone())
            || [
                row.x_um,
                row.y_um,
                row.outcome,
                row.resource_density,
                row.accessibility,
            ]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(BayesError::InvalidSpec(
                "distance observations require unique IDs and finite exact fields".into(),
            ));
        }
        *patient_counts.entry(row.patient_id.clone()).or_default() += 1;
    }
    if patient_counts.len() < 3 || patient_counts.values().any(|count| *count < 2) {
        return Err(BayesError::InvalidSpec(
            "distance hierarchy requires at least three patients with two observations each".into(),
        ));
    }
    Ok(())
}

fn invalid_id(value: &str) -> bool {
    value.is_empty() || value.trim() != value
}

fn derive_distances(spec: &DistanceToResourceSpec) -> Vec<DerivedResourceDistance> {
    spec.observations
        .iter()
        .map(|row| {
            let (resource, distance) = spec
                .resources
                .iter()
                .map(|resource| {
                    (
                        resource,
                        point_segment_distance(row.x_um, row.y_um, resource),
                    )
                })
                .min_by(|left, right| {
                    left.1
                        .total_cmp(&right.1)
                        .then_with(|| left.0.resource_id.cmp(&right.0.resource_id))
                })
                .expect("validated nonempty resources");
            DerivedResourceDistance {
                cell_id: row.cell_id.clone(),
                nearest_resource_id: resource.resource_id.clone(),
                unsigned_distance_um: distance,
            }
        })
        .collect()
}

fn point_segment_distance(x: f64, y: f64, resource: &ResourceSegment) -> f64 {
    let dx = resource.end_x_um - resource.start_x_um;
    let dy = resource.end_y_um - resource.start_y_um;
    let fraction = (((x - resource.start_x_um) * dx + (y - resource.start_y_um) * dy)
        / (dx * dx + dy * dy))
        .clamp(0.0, 1.0);
    (x - (resource.start_x_um + fraction * dx)).hypot(y - (resource.start_y_um + fraction * dy))
}

fn fixed_names(knots: &[f64], compartments: &[String]) -> Vec<String> {
    let mut names = vec!["intercept".into(), "distance_um".into()];
    names.extend(knots.iter().map(|knot| format!("distance_hinge_{knot}_um")));
    names.extend(
        compartments
            .iter()
            .skip(1)
            .map(|name| format!("compartment_{name}")),
    );
    names.extend(["resource_density".into(), "accessibility".into()]);
    names
}

fn features(
    row: &DistanceOutcomeObservation,
    distance: f64,
    knots: &[f64],
    compartments: &[String],
    patients: &BTreeMap<&str, usize>,
    fixed: usize,
) -> Vec<f64> {
    let mut values = Vec::with_capacity(fixed + patients.len());
    values.extend([1.0, distance]);
    values.extend(knots.iter().map(|knot| (distance - knot).max(0.0)));
    values.extend(
        compartments
            .iter()
            .skip(1)
            .map(|name| f64::from(row.compartment == *name)),
    );
    values.extend([row.resource_density, row.accessibility]);
    values.resize(fixed + patients.len(), 0.0);
    values[fixed + patients[row.patient_id.as_str()]] = 1.0;
    values
}

fn summary(name: String, mean: f64, variance: f64) -> GaussianPosteriorSummary {
    let sd = variance.max(0.0).sqrt();
    GaussianPosteriorSummary {
        name,
        mean,
        sd,
        interval_lower: mean - 1.959963984540054 * sd,
        interval_upper: mean + 1.959963984540054 * sd,
    }
}

fn predictions(
    spec: &DistanceToResourceSpec,
    design: &[f64],
    dimension: usize,
    mean: &[f64],
    covariance: &[f64],
) -> Vec<DistancePrediction> {
    spec.observations
        .iter()
        .zip(design.chunks_exact(dimension))
        .map(|(row, features)| {
            let (prediction, latent_sd) = linear_summary(features, mean, covariance);
            DistancePrediction {
                cell_id: row.cell_id.clone(),
                patient_id: row.patient_id.clone(),
                observed: row.outcome,
                posterior_predictive_mean: prediction,
                posterior_predictive_sd: latent_sd.hypot(spec.known_noise_sd),
                residual: row.outcome - prediction,
            }
        })
        .collect()
}

fn predictive_checks(
    predictions: &[DistancePrediction],
    distances: &[DerivedResourceDistance],
) -> DistancePosteriorPredictive {
    let rmse = (predictions
        .iter()
        .map(|row| row.residual.powi(2))
        .sum::<f64>()
        / predictions.len() as f64)
        .sqrt();
    let mean_residual =
        predictions.iter().map(|row| row.residual).sum::<f64>() / predictions.len() as f64;
    let mut grouped = BTreeMap::<String, Vec<&DistancePrediction>>::new();
    for row in predictions {
        grouped.entry(row.patient_id.clone()).or_default().push(row);
    }
    let patient_checks = grouped
        .into_iter()
        .map(|(patient_id, rows)| {
            let count = rows.len() as f64;
            PatientPredictiveCheck {
                patient_id,
                observations: rows.len() as u32,
                observed_mean: rows.iter().map(|row| row.observed).sum::<f64>() / count,
                posterior_predictive_mean: rows
                    .iter()
                    .map(|row| row.posterior_predictive_mean)
                    .sum::<f64>()
                    / count,
                mean_residual: rows.iter().map(|row| row.residual).sum::<f64>() / count,
            }
        })
        .collect();
    let mut grouped_resources = BTreeMap::<String, Vec<&DistancePrediction>>::new();
    for (prediction, distance) in predictions.iter().zip(distances) {
        grouped_resources
            .entry(distance.nearest_resource_id.clone())
            .or_default()
            .push(prediction);
    }
    let resource_checks = grouped_resources
        .into_iter()
        .map(|(resource_id, rows)| {
            let count = rows.len() as f64;
            ResourcePredictiveCheck {
                resource_id,
                observations: rows.len() as u32,
                observed_mean: rows.iter().map(|row| row.observed).sum::<f64>() / count,
                posterior_predictive_mean: rows
                    .iter()
                    .map(|row| row.posterior_predictive_mean)
                    .sum::<f64>()
                    / count,
                mean_residual: rows.iter().map(|row| row.residual).sum::<f64>() / count,
            }
        })
        .collect();
    DistancePosteriorPredictive {
        rmse,
        mean_residual,
        patient_checks,
        resource_checks,
    }
}

fn response_curve(
    spec: &DistanceToResourceSpec,
    distances: &[DerivedResourceDistance],
    compartments: &[String],
    dimension: usize,
    mean: &[f64],
    covariance: &[f64],
) -> Vec<DistanceResponsePoint> {
    let maximum = distances
        .iter()
        .map(|row| row.unsigned_distance_um)
        .fold(0.0, f64::max);
    let mean_density = spec
        .observations
        .iter()
        .map(|row| row.resource_density)
        .sum::<f64>()
        / spec.observations.len() as f64;
    let mean_accessibility = spec
        .observations
        .iter()
        .map(|row| row.accessibility)
        .sum::<f64>()
        / spec.observations.len() as f64;
    (0..=20)
        .map(|index| {
            let distance = maximum * f64::from(index) / 20.0;
            let row = DistanceOutcomeObservation {
                cell_id: String::new(),
                patient_id: String::new(),
                x_um: 0.0,
                y_um: 0.0,
                outcome: 0.0,
                compartment: compartments[0].clone(),
                resource_density: mean_density,
                accessibility: mean_accessibility,
            };
            let mut values = vec![1.0, distance];
            values.extend(
                spec.spline_knots_um
                    .iter()
                    .map(|knot| (distance - knot).max(0.0)),
            );
            values.extend(compartments.iter().skip(1).map(|_| 0.0));
            values.extend([row.resource_density, row.accessibility]);
            values.resize(dimension, 0.0);
            let (location, sd) = linear_summary(&values, mean, covariance);
            DistanceResponsePoint {
                distance_um: distance,
                posterior_mean: location,
                posterior_sd: sd,
                interval_lower: location - 1.959963984540054 * sd,
                interval_upper: location + 1.959963984540054 * sd,
            }
        })
        .collect()
}
