use crate::{
    classical::SpatialGeometryPlan2D, common::finite::canonical_zero,
    continuous_mark_correlation::compensated_add, ClassicalSpatialLimits,
};

use super::{
    analysis::{dependency, Counters},
    types::{
        InhomogeneousSpatialConfig, InhomogeneousSpatialError, InhomogeneousSpatialPoint,
        InhomogeneousSpatialPointStatus,
    },
};

pub(super) struct Evaluation {
    pub(super) points: Vec<InhomogeneousSpatialPoint>,
    pub(super) values: Vec<f64>,
    pub(super) eligible: Vec<bool>,
    pub(super) pair_visits: usize,
}

pub(super) fn evaluate_curve(
    plan: &SpatialGeometryPlan2D,
    intensities: &[f64],
    radii_um: &[f64],
    maximum_pair_visits: usize,
    counters: &mut Counters,
) -> Result<Evaluation, InhomogeneousSpatialError> {
    let width = radii_um
        .len()
        .checked_add(1)
        .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
    let mut eligible_ends = zeroed_vec::<usize>(width)?;
    let mut center_inverse_ends = zeroed_vec::<f64>(width)?;
    let mut center_inverse_end_corrections = zeroed_vec::<f64>(width)?;
    let mut pair_starts = zeroed_vec::<usize>(width)?;
    let mut pair_ends = zeroed_vec::<usize>(width)?;
    let mut weight_starts = zeroed_vec::<f64>(width)?;
    let mut weight_start_corrections = zeroed_vec::<f64>(width)?;
    let mut weight_ends = zeroed_vec::<f64>(width)?;
    let mut weight_end_corrections = zeroed_vec::<f64>(width)?;
    // Bin events are added and removed. Preserve Neumaier residuals through
    // cancellation instead of applying the final-total rule of a Kahan sum.
    let mut center_inverse_sum = 0.0;
    let mut center_inverse_correction = 0.0;
    let maximum_radius = radii_um[radii_um.len() - 1];
    let starting_visits = counters.pair_visits;
    for source in 0..intensities.len() {
        let inverse_source = 1.0 / intensities[source];
        compensated_add(
            &mut center_inverse_sum,
            &mut center_inverse_correction,
            inverse_source,
        );
        let end = radii_um.partition_point(|radius| *radius <= plan.boundary_distances()[source]);
        eligible_ends[end] = eligible_ends[end]
            .checked_add(1)
            .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
        compensated_add(
            &mut center_inverse_ends[end],
            &mut center_inverse_end_corrections[end],
            inverse_source,
        );
        let query_radius = maximum_radius.min(plan.boundary_distances()[source]);
        if query_radius < radii_um[0] {
            continue;
        }
        let mut visitor_error = None;
        plan.index()
            .visit_within_radius(source, query_radius, |neighbor| {
                if visitor_error.is_some() {
                    return;
                }
                if let Err(error) = counters.charge_pair(maximum_pair_visits) {
                    visitor_error = Some(error);
                    return;
                }
                let start = radii_um.partition_point(|radius| *radius < neighbor.distance_um);
                if start >= end || start >= radii_um.len() {
                    return;
                }
                pair_starts[start] = match pair_starts[start].checked_add(1) {
                    Some(value) => value,
                    None => {
                        visitor_error = Some(InhomogeneousSpatialError::SizeOverflow);
                        return;
                    }
                };
                pair_ends[end] = match pair_ends[end].checked_add(1) {
                    Some(value) => value,
                    None => {
                        visitor_error = Some(InhomogeneousSpatialError::SizeOverflow);
                        return;
                    }
                };
                let weight = inverse_source / intensities[neighbor.index];
                if !weight.is_finite() || weight <= 0.0 {
                    visitor_error = Some(InhomogeneousSpatialError::Dependency(
                        "inverse-intensity pair weight is invalid".into(),
                    ));
                    return;
                }
                compensated_add(
                    &mut weight_starts[start],
                    &mut weight_start_corrections[start],
                    weight,
                );
                compensated_add(
                    &mut weight_ends[end],
                    &mut weight_end_corrections[end],
                    weight,
                );
            })
            .map_err(dependency)?;
        if let Some(error) = visitor_error {
            return Err(error);
        }
    }

    let mut eligible_centers = intensities.len();
    let mut active_pairs = 0_usize;
    let mut active_weight = 0.0;
    let mut active_weight_correction = 0.0;
    let mut points = Vec::new();
    let mut values = Vec::new();
    let mut eligible = Vec::new();
    points
        .try_reserve_exact(radii_um.len())
        .and_then(|()| values.try_reserve_exact(radii_um.len()))
        .and_then(|()| eligible.try_reserve_exact(radii_um.len()))
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    for (index, radius_um) in radii_um.iter().copied().enumerate() {
        eligible_centers = eligible_centers
            .checked_sub(eligible_ends[index])
            .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
        compensated_add(
            &mut center_inverse_sum,
            &mut center_inverse_correction,
            -center_inverse_ends[index],
        );
        compensated_add(
            &mut center_inverse_sum,
            &mut center_inverse_correction,
            -center_inverse_end_corrections[index],
        );
        active_pairs = active_pairs
            .checked_sub(pair_ends[index])
            .and_then(|value| value.checked_add(pair_starts[index]))
            .ok_or(InhomogeneousSpatialError::SizeOverflow)?;
        compensated_add(
            &mut active_weight,
            &mut active_weight_correction,
            -weight_ends[index],
        );
        compensated_add(
            &mut active_weight,
            &mut active_weight_correction,
            -weight_end_corrections[index],
        );
        compensated_add(
            &mut active_weight,
            &mut active_weight_correction,
            weight_starts[index],
        );
        compensated_add(
            &mut active_weight,
            &mut active_weight_correction,
            weight_start_corrections[index],
        );
        let center_sum = center_inverse_sum + center_inverse_correction;
        let pair_sum = if active_pairs == 0 {
            0.0
        } else {
            active_weight + active_weight_correction
        };
        let available = eligible_centers > 0 && center_sum.is_finite() && center_sum > 0.0;
        let k = available.then(|| pair_sum / center_sum).map(canonical_zero);
        let l = k.map(|value| (value / std::f64::consts::PI).sqrt());
        if k.is_some_and(|value| !value.is_finite() || value < 0.0)
            || l.is_some_and(|value| !value.is_finite())
        {
            return Err(InhomogeneousSpatialError::Dependency(
                "inhomogeneous K/L normalization is invalid".into(),
            ));
        }
        values.push(l.unwrap_or(0.0));
        eligible.push(l.is_some());
        points.push(InhomogeneousSpatialPoint {
            radius_um,
            status: if available {
                InhomogeneousSpatialPointStatus::Available
            } else {
                InhomogeneousSpatialPointStatus::NoEligibleCenters
            },
            eligible_centers,
            directed_pairs: active_pairs,
            inverse_intensity_pair_sum: canonical_zero(pair_sum),
            eligible_center_inverse_intensity_sum: canonical_zero(center_sum),
            k,
            l,
            theoretical_k: std::f64::consts::PI * radius_um * radius_um,
            theoretical_l: radius_um,
            inference_eligible: false,
            lower_l: None,
            upper_l: None,
        });
    }
    Ok(Evaluation {
        points,
        values,
        eligible,
        pair_visits: counters.pair_visits - starting_visits,
    })
}

pub(super) fn geometry_limits(
    config: &InhomogeneousSpatialConfig,
) -> Result<ClassicalSpatialLimits, InhomogeneousSpatialError> {
    ClassicalSpatialLimits::new(
        config.limits.maximum_points,
        config.limits.maximum_radii,
        config.limits.maximum_pair_visits,
        config.limits.maximum_null_draws,
        config.limits.maximum_retained_bytes,
    )
    .map_err(dependency)
}

fn zeroed_vec<T: Default + Clone>(length: usize) -> Result<Vec<T>, InhomogeneousSpatialError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| InhomogeneousSpatialError::AllocationFailed)?;
    values.resize(length, T::default());
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ObservationWindowLimits;

    #[test]
    fn border_kl_retains_small_center_weights_after_a_large_weight_exits() {
        let window = crate::ObservationWindow2D::from_geojson_str(
            r#"{"type":"MultiPolygon","coordinates":[[[[0,0],[10,0],[10,10],[0,10],[0,0]]]]}"#,
            ObservationWindowLimits::default(),
        )
        .unwrap();
        for (x, intensities, radii, expected_centers, expected_pairs) in [
            (
                vec![0.25, 5.0, 6.0],
                vec![1e-16, 1.0, 0.5],
                vec![1.1],
                3.0,
                4.0,
            ),
            (
                vec![0.25, 0.35, 5.0, 5.1],
                vec![1e-16, 1.0, 1.0, 1.0],
                vec![0.2, 1.1],
                2.0,
                2.0,
            ),
        ] {
            let plan = SpatialGeometryPlan2D::new(
                &x,
                &vec![5.0; x.len()],
                &window,
                ClassicalSpatialLimits::new(4, 2, 12, 1, 1 << 20).unwrap(),
            )
            .unwrap();
            let mut counters = Counters {
                intensity_evaluations: 0,
                pair_visits: 0,
                null_draws: 0,
            };
            let result = evaluate_curve(&plan, &intensities, &radii, 12, &mut counters).unwrap();
            let point = result.points.last().unwrap();
            // At the final radius only the two interior centers and their
            // two ordered pairs remain; large boundary weights must cancel.
            assert_eq!(point.eligible_centers, 2);
            assert_eq!(point.directed_pairs, 2);
            assert_eq!(
                point.eligible_center_inverse_intensity_sum,
                expected_centers
            );
            assert_eq!(point.inverse_intensity_pair_sum, expected_pairs);
            assert!((point.k.unwrap() - expected_pairs / expected_centers).abs() < 1e-15);
        }
    }
}
