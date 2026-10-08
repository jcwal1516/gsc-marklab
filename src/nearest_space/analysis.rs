use marklab_workflow::ContentDigest;

use crate::common::finite::canonical_zero;

use crate::{
    classical::{sample_conditional_csr, window_summary, SpatialGeometryPlan2D},
    common::seeds::{derive_seed, SeedEndpoint},
    geom::window::ObservationWindow2D,
    permutation::envelopes::GlobalEnvelope,
    ClassicalNullDesign, ClassicalNullModel, ClassicalRandomizationUnit, ClassicalSpatialLimits,
    Pattern,
};

use super::types::*;

struct Probe {
    x_um: f64,
    y_um: f64,
    boundary_distance_um: f64,
}

struct ProbePlan {
    probes: Vec<Probe>,
    summary: NearestSpaceProbeSummary,
}

struct CurveEvaluation {
    points: Vec<NearestSpacePoint>,
    f: Vec<f64>,
    g: Vec<f64>,
    j: Vec<f64>,
    f_eligible: Vec<bool>,
    g_eligible: Vec<bool>,
    j_eligible: Vec<bool>,
}

/// Compute exact reduced-sample F/G/J curves with whole-pattern conditional-CSR inference.
pub fn analyze_nearest_space_pattern(
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &NearestSpaceConfig,
) -> Result<NearestSpaceResult, NearestSpaceError> {
    validate_pattern_shape(pattern)?;
    if pattern.len() > config.limits.maximum_points {
        return Err(NearestSpaceError::PointLimitExceeded {
            observed: pattern.len(),
            maximum: config.limits.maximum_points,
        });
    }
    let requested_probe_bytes = config.probe_grid[0]
        .checked_mul(config.probe_grid[1])
        .and_then(|count| count.checked_mul(std::mem::size_of::<Probe>()))
        .ok_or(NearestSpaceError::SizeOverflow)?;
    if requested_probe_bytes > config.limits.maximum_retained_bytes {
        return Err(NearestSpaceError::RetainedByteLimitExceeded {
            required: requested_probe_bytes,
            maximum: config.limits.maximum_retained_bytes,
        });
    }
    let probes = build_probe_plan(window, config)?;
    let retained = retained_byte_estimate(pattern.len(), probes.probes.len(), window, config)?;
    if retained > config.limits.maximum_retained_bytes {
        return Err(NearestSpaceError::RetainedByteLimitExceeded {
            required: retained,
            maximum: config.limits.maximum_retained_bytes,
        });
    }
    let classical_limits = ClassicalSpatialLimits::new(
        config.limits.maximum_points,
        config.limits.maximum_radii,
        config.limits.maximum_nearest_queries,
        config.limits.maximum_csr_draws,
        config.limits.maximum_retained_bytes,
    )
    .map_err(geometry_error)?;
    let observed_plan =
        SpatialGeometryPlan2D::new(&pattern.x_um, &pattern.y_um, window, classical_limits)
            .map_err(geometry_error)?;
    let mut nearest_queries = 0_usize;
    let mut observed = evaluate_curve(
        &observed_plan,
        &probes.probes,
        &config.radii_um,
        config.j_denominator_epsilon,
        &mut nearest_queries,
        config.limits.maximum_nearest_queries,
    )?;

    let mut simulated_f = Vec::new();
    let mut simulated_g = Vec::new();
    let mut simulated_j = Vec::new();
    simulated_f
        .try_reserve_exact(config.simulations)
        .and_then(|()| simulated_g.try_reserve_exact(config.simulations))
        .and_then(|()| simulated_j.try_reserve_exact(config.simulations))
        .map_err(|_| NearestSpaceError::AllocationFailed)?;
    let mut f_eligible = observed.f_eligible.clone();
    let mut g_eligible = observed.g_eligible.clone();
    let mut j_eligible = observed.j_eligible.clone();
    let mut csr_draws = 0_usize;
    if !pattern.is_empty() {
        for simulation in 0..config.simulations {
            let seed = derive_seed(config.seed, SeedEndpoint::NearestSpaceCsr, simulation);
            let (x, y) = sample_conditional_csr(
                window,
                pattern.len(),
                seed,
                &mut csr_draws,
                config.limits.maximum_csr_draws,
            )
            .map_err(map_csr_error)?;
            let plan = SpatialGeometryPlan2D::new(&x, &y, window, classical_limits)
                .map_err(geometry_error)?;
            let evaluated = evaluate_curve(
                &plan,
                &probes.probes,
                &config.radii_um,
                config.j_denominator_epsilon,
                &mut nearest_queries,
                config.limits.maximum_nearest_queries,
            )?;
            intersect_eligibility(&mut f_eligible, &evaluated.f_eligible);
            intersect_eligibility(&mut g_eligible, &evaluated.g_eligible);
            intersect_eligibility(&mut j_eligible, &evaluated.j_eligible);
            simulated_f.push(evaluated.f);
            simulated_g.push(evaluated.g);
            simulated_j.push(evaluated.j);
        }
    } else {
        f_eligible.fill(false);
        g_eligible.fill(false);
        j_eligible.fill(false);
    }

    let f_inference = attach_envelope(
        &mut observed.points,
        CurveComponent::F,
        &observed.f,
        &simulated_f,
        &f_eligible,
        config.alpha,
    )?;
    let g_inference = attach_envelope(
        &mut observed.points,
        CurveComponent::G,
        &observed.g,
        &simulated_g,
        &g_eligible,
        config.alpha,
    )?;
    let j_inference = attach_envelope(
        &mut observed.points,
        CurveComponent::J,
        &observed.j,
        &simulated_j,
        &j_eligible,
        config.alpha,
    )?;
    let inference = NearestSpaceInferenceSummary {
        f: f_inference,
        g: g_inference,
        j: j_inference,
    };
    let status = if pattern.len() < 2 {
        NearestSpaceStatus::InsufficientEvents
    } else if inference.f.is_some() || inference.g.is_some() || inference.j.is_some() {
        NearestSpaceStatus::Available
    } else {
        NearestSpaceStatus::InsufficientInferenceSupport
    };
    let configuration_digest = configuration_digest(config);
    Ok(NearestSpaceResult {
        case_id: pattern.meta.case_id.clone(),
        timepoint: pattern.meta.timepoint.clone(),
        status,
        window: window_summary(window.descriptor()),
        probes: probes.summary,
        geometry: NearestSpaceGeometrySummary {
            point_count: pattern.len(),
            maximum_radius_um: *config.radii_um.last().expect("validated radii"),
            nearest_query_count: nearest_queries,
            estimated_storage_bytes: retained,
            logical_digest: observed_plan.logical_digest().to_string(),
            nearest_neighbor_owner: "spatial_index_2d".into(),
            edge_correction: "standard_border_reduced_sample".into(),
        },
        configuration: NearestSpaceConfigurationSummary {
            radius_count: config.radii_um.len(),
            j_denominator_epsilon: config.j_denominator_epsilon,
            logical_digest: configuration_digest.to_string(),
            limits: config.limits,
        },
        null_design: ClassicalNullDesign {
            null_model: ClassicalNullModel::HomogeneousCsrConditionalOnCount,
            randomization_unit: ClassicalRandomizationUnit::WholeLocationPattern,
            conditioned_point_count: pattern.len(),
            simulations: config.simulations,
            seed: config.seed,
            alpha: config.alpha,
        },
        curve: observed.points,
        inference,
        csr_candidate_draws: csr_draws,
    })
}

fn evaluate_curve(
    plan: &SpatialGeometryPlan2D,
    probes: &[Probe],
    radii_um: &[f64],
    j_epsilon: f64,
    query_count: &mut usize,
    maximum_queries: usize,
) -> Result<CurveEvaluation, NearestSpaceError> {
    let event_distances = if plan.index().len() >= 2 {
        let mut distances = Vec::new();
        distances
            .try_reserve_exact(plan.index().len())
            .map_err(|_| NearestSpaceError::AllocationFailed)?;
        for row in 0..plan.index().len() {
            charge_query(query_count, maximum_queries)?;
            distances.push(
                plan.index()
                    .nearest_neighbor(row)
                    .map_err(geometry_error)?
                    .ok_or_else(|| NearestSpaceError::Geometry {
                        reason: "event nearest-neighbour query returned no distinct event".into(),
                    })?
                    .distance_um,
            );
        }
        Some(distances)
    } else {
        None
    };
    let probe_distances = if plan.index().len() >= 1 {
        let mut distances = Vec::new();
        distances
            .try_reserve_exact(probes.len())
            .map_err(|_| NearestSpaceError::AllocationFailed)?;
        for probe in probes {
            charge_query(query_count, maximum_queries)?;
            distances.push(
                plan.index()
                    .nearest_point(probe.x_um, probe.y_um)
                    .map_err(geometry_error)?
                    .ok_or_else(|| NearestSpaceError::Geometry {
                        reason: "probe nearest-event query returned no event".into(),
                    })?
                    .distance_um,
            );
        }
        Some(distances)
    } else {
        None
    };

    let mut points = Vec::new();
    let mut f_values = Vec::new();
    let mut g_values = Vec::new();
    let mut j_values = Vec::new();
    points
        .try_reserve_exact(radii_um.len())
        .and_then(|()| f_values.try_reserve_exact(radii_um.len()))
        .and_then(|()| g_values.try_reserve_exact(radii_um.len()))
        .and_then(|()| j_values.try_reserve_exact(radii_um.len()))
        .map_err(|_| NearestSpaceError::AllocationFailed)?;
    let mut f_eligible = Vec::with_capacity(radii_um.len());
    let mut g_eligible = Vec::with_capacity(radii_um.len());
    let mut j_eligible = Vec::with_capacity(radii_um.len());
    for &radius_um in radii_um {
        let eligible_probes = probes
            .iter()
            .filter(|probe| probe.boundary_distance_um >= radius_um)
            .count();
        let probes_with_event = probe_distances.as_ref().map_or(0, |distances| {
            probes
                .iter()
                .zip(distances)
                .filter(|(probe, distance)| {
                    probe.boundary_distance_um >= radius_um && **distance <= radius_um
                })
                .count()
        });
        let f = (eligible_probes > 0 && probe_distances.is_some())
            .then(|| canonical_zero(probes_with_event as f64 / eligible_probes as f64));
        let f_status = if plan.index().len() == 0 {
            DistributionPointStatus::InsufficientEvents
        } else if f.is_some() {
            DistributionPointStatus::Available
        } else {
            DistributionPointStatus::NoEligibleCenters
        };

        let eligible_events = plan
            .boundary_distances()
            .iter()
            .filter(|distance| **distance >= radius_um)
            .count();
        let events_with_neighbor = event_distances.as_ref().map_or(0, |distances| {
            plan.boundary_distances()
                .iter()
                .zip(distances)
                .filter(|(boundary, distance)| **boundary >= radius_um && **distance <= radius_um)
                .count()
        });
        let g = (eligible_events > 0 && event_distances.is_some())
            .then(|| canonical_zero(events_with_neighbor as f64 / eligible_events as f64));
        let g_status = if plan.index().len() < 2 {
            DistributionPointStatus::InsufficientEvents
        } else if g.is_some() {
            DistributionPointStatus::Available
        } else {
            DistributionPointStatus::NoEligibleCenters
        };
        let (j_status, j) = match (f, g) {
            (Some(f), Some(g)) if 1.0 - f > j_epsilon => (
                JPointStatus::Available,
                Some(canonical_zero((1.0 - g) / (1.0 - f))),
            ),
            (Some(_), Some(_)) => (JPointStatus::DenominatorTooSmall, None),
            _ => (JPointStatus::MissingComponent, None),
        };
        f_values.push(f.unwrap_or(0.0));
        g_values.push(g.unwrap_or(0.0));
        j_values.push(j.unwrap_or(0.0));
        f_eligible.push(f.is_some());
        g_eligible.push(g.is_some());
        j_eligible.push(j.is_some());
        points.push(NearestSpacePoint {
            radius_um,
            f_status,
            g_status,
            j_status,
            eligible_probes,
            probes_with_event_within_radius: probes_with_event,
            eligible_event_centers: eligible_events,
            events_with_neighbor_within_radius: events_with_neighbor,
            f,
            g,
            j,
            f_inference_eligible: false,
            lower_f: None,
            upper_f: None,
            g_inference_eligible: false,
            lower_g: None,
            upper_g: None,
            j_inference_eligible: false,
            lower_j: None,
            upper_j: None,
        });
    }
    Ok(CurveEvaluation {
        points,
        f: f_values,
        g: g_values,
        j: j_values,
        f_eligible,
        g_eligible,
        j_eligible,
    })
}

#[derive(Clone, Copy)]
enum CurveComponent {
    F,
    G,
    J,
}

fn attach_envelope(
    points: &mut [NearestSpacePoint],
    component: CurveComponent,
    observed: &[f64],
    simulations: &[Vec<f64>],
    eligible: &[bool],
    alpha: f64,
) -> Result<Option<NearestSpaceComponentInference>, NearestSpaceError> {
    if simulations.is_empty() || !eligible.iter().any(|value| *value) {
        return Ok(None);
    }
    let envelope =
        GlobalEnvelope::from_curves_with_eligibility(observed, simulations, alpha, eligible)
            .map_err(|error| NearestSpaceError::Inference {
                reason: error.to_string(),
            })?;
    for (index, point) in points.iter_mut().enumerate() {
        if !eligible[index] {
            continue;
        }
        let lower = canonical_zero(envelope.lower[index]);
        let upper = canonical_zero(envelope.upper[index]);
        match component {
            CurveComponent::F => {
                point.f_inference_eligible = true;
                point.lower_f = Some(lower);
                point.upper_f = Some(upper);
            }
            CurveComponent::G => {
                point.g_inference_eligible = true;
                point.lower_g = Some(lower);
                point.upper_g = Some(upper);
            }
            CurveComponent::J => {
                point.j_inference_eligible = true;
                point.lower_j = Some(lower);
                point.upper_j = Some(upper);
            }
        }
    }
    Ok(Some(NearestSpaceComponentInference {
        p_global: canonical_zero(envelope.p_global),
        erl_depth: canonical_zero(envelope.erl_depth),
        critical_depth: canonical_zero(envelope.critical_depth),
        eligible_radius_count: eligible.iter().filter(|value| **value).count(),
    }))
}

fn build_probe_plan(
    window: &ObservationWindow2D,
    config: &NearestSpaceConfig,
) -> Result<ProbePlan, NearestSpaceError> {
    let [min_x, min_y, max_x, max_y] = window.descriptor().bounds_um;
    let spacing_x = (max_x - min_x) / config.probe_grid[0] as f64;
    let spacing_y = (max_y - min_y) / config.probe_grid[1] as f64;
    if !spacing_x.is_finite() || !spacing_y.is_finite() || spacing_x <= 0.0 || spacing_y <= 0.0 {
        return Err(NearestSpaceError::Geometry {
            reason: "window bounds do not define a positive finite probe grid".into(),
        });
    }
    let mut probes = Vec::new();
    probes
        .try_reserve_exact(config.probe_grid[0].saturating_mul(config.probe_grid[1]))
        .map_err(|_| NearestSpaceError::AllocationFailed)?;
    for y_index in 0..config.probe_grid[1] {
        let y_um = min_y + (y_index as f64 + 0.5) * spacing_y;
        for x_index in 0..config.probe_grid[0] {
            let x_um = min_x + (x_index as f64 + 0.5) * spacing_x;
            if window.contains(x_um, y_um) {
                probes.push(Probe {
                    x_um,
                    y_um,
                    boundary_distance_um: window
                        .boundary_distance_um(x_um, y_um)
                        .map_err(geometry_error)?,
                });
            }
        }
    }
    if probes.is_empty() {
        return Err(NearestSpaceError::NoProbesInsideWindow);
    }
    let grid_x = (config.probe_grid[0] as u128).to_be_bytes();
    let grid_y = (config.probe_grid[1] as u128).to_be_bytes();
    let logical_digest = ContentDigest::from_framed([
        b"marklab-nearest-space-fixed-probes-v1".as_slice(),
        window.descriptor().logical_digest.as_bytes(),
        &grid_x,
        &grid_y,
    ]);
    Ok(ProbePlan {
        summary: NearestSpaceProbeSummary {
            requested_grid: config.probe_grid,
            retained_probe_count: probes.len(),
            spacing_um: [spacing_x, spacing_y],
            maximum_location_error_um: canonical_zero(spacing_x.hypot(spacing_y) / 2.0),
            logical_digest: logical_digest.to_string(),
            method: "fixed_cell_centred_rectangular_grid".into(),
        },
        probes,
    })
}

fn retained_byte_estimate(
    point_count: usize,
    probe_count: usize,
    window: &ObservationWindow2D,
    config: &NearestSpaceConfig,
) -> Result<usize, NearestSpaceError> {
    let plan =
        crate::geom::spatial_index::SpatialIndex2D::estimated_storage_bytes_for_len(point_count)
            .checked_add(
                point_count
                    .checked_mul(std::mem::size_of::<f64>())
                    .ok_or(NearestSpaceError::SizeOverflow)?,
            )
            .ok_or(NearestSpaceError::SizeOverflow)?;
    let generated = point_count
        .checked_mul(2)
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(NearestSpaceError::SizeOverflow)?;
    let coordinate_set_work = point_count
        .checked_mul(std::mem::size_of::<((u64, u64), usize)>())
        .and_then(|value| value.checked_mul(4))
        .ok_or(NearestSpaceError::SizeOverflow)?;
    let probes = probe_count
        .checked_mul(std::mem::size_of::<Probe>() + std::mem::size_of::<f64>())
        .ok_or(NearestSpaceError::SizeOverflow)?;
    let matrices = config
        .simulations
        .checked_mul(config.radii_um.len())
        .and_then(|value| value.checked_mul(3))
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(NearestSpaceError::SizeOverflow)?;
    let curve_work = config
        .radii_um
        .len()
        .checked_mul(
            std::mem::size_of::<NearestSpacePoint>()
                + 3 * std::mem::size_of::<f64>()
                + 3 * std::mem::size_of::<bool>(),
        )
        .ok_or(NearestSpaceError::SizeOverflow)?;
    plan.checked_mul(2)
        .and_then(|value| value.checked_add(window.boundary_storage_bytes()))
        .and_then(|value| value.checked_add(generated))
        .and_then(|value| value.checked_add(coordinate_set_work))
        .and_then(|value| value.checked_add(probes))
        .and_then(|value| value.checked_add(matrices))
        .and_then(|value| value.checked_add(curve_work))
        .ok_or(NearestSpaceError::SizeOverflow)
}

pub(crate) fn configuration_digest(config: &NearestSpaceConfig) -> ContentDigest {
    let mut fields = vec![
        b"marklab-nearest-space-configuration-v1".to_vec(),
        (config.radii_um.len() as u128).to_be_bytes().to_vec(),
    ];
    for radius in &config.radii_um {
        fields.push(radius.to_bits().to_be_bytes().to_vec());
    }
    fields.extend([
        (config.probe_grid[0] as u128).to_be_bytes().to_vec(),
        (config.probe_grid[1] as u128).to_be_bytes().to_vec(),
        (config.simulations as u128).to_be_bytes().to_vec(),
        config.seed.to_be_bytes().to_vec(),
        config.alpha.to_bits().to_be_bytes().to_vec(),
        config
            .j_denominator_epsilon
            .to_bits()
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_points as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_radii as u128).to_be_bytes().to_vec(),
        (config.limits.maximum_probes as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_nearest_queries as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_csr_draws as u128)
            .to_be_bytes()
            .to_vec(),
        (config.limits.maximum_retained_bytes as u128)
            .to_be_bytes()
            .to_vec(),
    ]);
    ContentDigest::from_framed(fields.iter().map(Vec::as_slice))
}

fn validate_pattern_shape(pattern: &Pattern) -> Result<(), NearestSpaceError> {
    if pattern.x_um.len() != pattern.len()
        || pattern.y_um.len() != pattern.len()
        || pattern.valid.len() != pattern.len()
        || pattern.valid.iter().any(|value| *value != 1)
    {
        return Err(NearestSpaceError::PatternShapeMismatch);
    }
    Ok(())
}

fn charge_query(count: &mut usize, maximum: usize) -> Result<(), NearestSpaceError> {
    *count = count
        .checked_add(1)
        .ok_or(NearestSpaceError::SizeOverflow)?;
    if *count > maximum {
        return Err(NearestSpaceError::NearestQueryLimitExceeded { maximum });
    }
    Ok(())
}

fn intersect_eligibility(target: &mut [bool], observed: &[bool]) {
    for (target, observed) in target.iter_mut().zip(observed) {
        *target &= *observed;
    }
}

fn map_csr_error(error: crate::ClassicalSpatialError) -> NearestSpaceError {
    match error {
        crate::ClassicalSpatialError::CsrDrawLimitExceeded { maximum } => {
            NearestSpaceError::CsrDrawLimitExceeded { maximum }
        }
        error => geometry_error(error),
    }
}

fn geometry_error(error: impl std::fmt::Display) -> NearestSpaceError {
    NearestSpaceError::Geometry {
        reason: error.to_string(),
    }
}
