use crate::{
    geom::window::ObservationWindow2D,
    scalar_mark::{DeclaredScalarPatternInput, ScalarMarkId},
};

use super::{
    admission::{prepare, PreparedSpatialAutocorrelation},
    GlobalGearyAlternative, GlobalGearyDesign, GlobalGearyError, GlobalGearyLimits,
    GlobalGearyResult, GlobalMoranAlternative, GlobalMoranDesign, GlobalMoranError,
    GlobalMoranLimits, GlobalMoranResult, GlobalMoranWeightPolicy,
};

/// Compute global Moran's I and a deterministic whole-value random-labeling test.
#[allow(clippy::too_many_arguments)]
pub fn global_moran_permutation(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    radius_um: f64,
    weight_policy: GlobalMoranWeightPolicy,
    design: &GlobalMoranDesign,
    limits: GlobalMoranLimits,
) -> Result<GlobalMoranResult, GlobalMoranError> {
    let PreparedSpatialAutocorrelation {
        frame,
        measurement_status,
        row_count,
        values,
        weights,
        inference_design,
        conditioning_mark_id,
        conditioning_measurement_status,
    } = prepare(
        input,
        window,
        mark_id,
        radius_um,
        weight_policy,
        design,
        limits,
    )?;
    let statistic = weights.evaluate(&values)?;
    let null_expectation = -1.0 / (row_count as f64 - 1.0);
    let mut extreme = 0_usize;
    for replicate in 0..design.permutations {
        let permuted = inference_design
            .permuted_indices(replicate)
            .map_err(|error| GlobalMoranError::InvalidDesign(error.to_string()))?
            .iter()
            .map(|source| values[*source])
            .collect::<Vec<_>>();
        let candidate = weights.evaluate(&permuted)?;
        extreme += usize::from(match design.alternative {
            GlobalMoranAlternative::Less => candidate <= statistic,
            GlobalMoranAlternative::Greater => candidate >= statistic,
            GlobalMoranAlternative::TwoSided => {
                (candidate - null_expectation).abs() >= (statistic - null_expectation).abs()
            }
        });
    }
    let p_value = (extreme as f64 + 1.0) / (design.permutations as f64 + 1.0);
    if !p_value.is_finite() || !(0.0..=1.0).contains(&p_value) {
        return Err(GlobalMoranError::NumericalFailure);
    }

    Ok(GlobalMoranResult {
        mark_id: mark_id.clone(),
        measurement_status,
        coordinate_frame_id: frame.clone(),
        radius_um,
        weight_policy,
        weights_digest: weights.digest,
        point_count: row_count,
        directed_edge_count: weights.edges.len(),
        stratum_count: inference_design.block_count(),
        conditioning_mark_id,
        conditioning_measurement_status,
        statistic,
        null_expectation,
        p_value,
        permutations_requested: design.permutations,
        permutations_attempted: design.permutations,
        permutations_completed: design.permutations,
        seed: design.seed,
        alternative: design.alternative,
        conditioning: design.conditioning,
    })
}

/// Compute global Geary's C and a deterministic whole-value random-labeling test.
#[allow(clippy::too_many_arguments)]
pub fn global_geary_permutation(
    input: &DeclaredScalarPatternInput<'_>,
    window: &ObservationWindow2D,
    mark_id: &ScalarMarkId,
    radius_um: f64,
    weight_policy: GlobalMoranWeightPolicy,
    design: &GlobalGearyDesign,
    limits: GlobalGearyLimits,
) -> Result<GlobalGearyResult, GlobalGearyError> {
    let PreparedSpatialAutocorrelation {
        frame,
        measurement_status,
        row_count,
        values,
        weights,
        inference_design,
        conditioning_mark_id,
        conditioning_measurement_status,
    } = prepare(
        input,
        window,
        mark_id,
        radius_um,
        weight_policy,
        design,
        limits,
    )?;
    let statistic = weights.evaluate_geary(&values)?;
    let null_expectation = 1.0;
    let mut extreme = 0_usize;
    for replicate in 0..design.permutations {
        let permuted = inference_design
            .permuted_indices(replicate)
            .map_err(|error| GlobalMoranError::InvalidDesign(error.to_string()))?
            .iter()
            .map(|source| values[*source])
            .collect::<Vec<_>>();
        let candidate = weights.evaluate_geary(&permuted)?;
        extreme += usize::from(match design.alternative {
            GlobalGearyAlternative::Less => candidate <= statistic,
            GlobalGearyAlternative::Greater => candidate >= statistic,
            GlobalGearyAlternative::TwoSided => {
                (candidate - null_expectation).abs() >= (statistic - null_expectation).abs()
            }
        });
    }
    let p_value = (extreme as f64 + 1.0) / (design.permutations as f64 + 1.0);
    if !p_value.is_finite() || !(0.0..=1.0).contains(&p_value) {
        return Err(GlobalMoranError::NumericalFailure);
    }

    Ok(GlobalGearyResult {
        mark_id: mark_id.clone(),
        measurement_status,
        coordinate_frame_id: frame.clone(),
        radius_um,
        weight_policy,
        weights_digest: weights.digest,
        point_count: row_count,
        directed_edge_count: weights.edges.len(),
        stratum_count: inference_design.block_count(),
        conditioning_mark_id,
        conditioning_measurement_status,
        statistic,
        null_expectation,
        p_value,
        permutations_requested: design.permutations,
        permutations_attempted: design.permutations,
        permutations_completed: design.permutations,
        seed: design.seed,
        alternative: design.alternative,
        conditioning: design.conditioning,
    })
}
