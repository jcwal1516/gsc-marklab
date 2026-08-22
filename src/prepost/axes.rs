use crate::output::{CrossInteractionCurve, MarkedPatternResult};

const AXIS_ABSOLUTE_TOLERANCE: f64 = 1e-12;
const AXIS_RELATIVE_TOLERANCE: f64 = 1e-12;

pub(super) fn spectrum_axes_aligned(
    pre: &MarkedPatternResult,
    post: &MarkedPatternResult,
) -> Result<(), String> {
    if pre.spectrum_curve.len() != post.spectrum_curve.len() {
        return Err(format!(
            "spectrum k-axis lengths differ: {} vs {}",
            pre.spectrum_curve.len(),
            post.spectrum_curve.len()
        ));
    }

    for (index, (pre_point, post_point)) in pre
        .spectrum_curve
        .iter()
        .zip(&post.spectrum_curve)
        .enumerate()
    {
        if !pre_point.k.is_finite() || !post_point.k.is_finite() {
            return Err(format!(
                "spectrum k-axis contains non-finite value at index {index}"
            ));
        }
        if !axis_values_match(pre_point.k, post_point.k) {
            return Err(format!(
                "spectrum k-axis differs at index {index}: {} vs {}",
                pre_point.k, post_point.k
            ));
        }
    }
    Ok(())
}

pub(super) fn mark_pair_covariance_axes_aligned(
    pre: &MarkedPatternResult,
    post: &MarkedPatternResult,
) -> Result<(), String> {
    if pre.mark_pair_covariance_curve.len() != post.mark_pair_covariance_curve.len() {
        return Err(format!(
            "mark-pair-covariance bin counts differ: {} vs {}",
            pre.mark_pair_covariance_curve.len(),
            post.mark_pair_covariance_curve.len()
        ));
    }

    for (index, (pre_point, post_point)) in pre
        .mark_pair_covariance_curve
        .iter()
        .zip(&post.mark_pair_covariance_curve)
        .enumerate()
    {
        if !pre_point.r_min_um.is_finite()
            || !pre_point.r_max_um.is_finite()
            || !post_point.r_min_um.is_finite()
            || !post_point.r_max_um.is_finite()
        {
            return Err(format!(
                "mark-pair-covariance axis contains non-finite bin edge at index {index}"
            ));
        }
        if !axis_values_match(pre_point.r_min_um, post_point.r_min_um)
            || !axis_values_match(pre_point.r_max_um, post_point.r_max_um)
        {
            return Err(format!(
                "mark-pair-covariance axis differs at index {index}: [{}, {}) vs [{}, {})",
                pre_point.r_min_um, pre_point.r_max_um, post_point.r_min_um, post_point.r_max_um
            ));
        }
        if pre_point.covariance.is_some() != post_point.covariance.is_some() {
            return Err(format!(
                "mark-pair-covariance availability differs at index {index}"
            ));
        }
    }
    Ok(())
}

pub(super) fn cross_interaction_axes_aligned(
    pre: &CrossInteractionCurve,
    post: &CrossInteractionCurve,
) -> Result<(), String> {
    if pre.points.len() != post.points.len() {
        return Err(format!(
            "cross-interaction bin counts differ: {} vs {}",
            pre.points.len(),
            post.points.len()
        ));
    }

    for (index, (pre_point, post_point)) in pre.points.iter().zip(&post.points).enumerate() {
        if !pre_point.r_min_um.is_finite()
            || !pre_point.r_max_um.is_finite()
            || !post_point.r_min_um.is_finite()
            || !post_point.r_max_um.is_finite()
        {
            return Err(format!(
                "cross-interaction axis contains non-finite bin edge at index {index}"
            ));
        }
        if !axis_values_match(pre_point.r_min_um, post_point.r_min_um)
            || !axis_values_match(pre_point.r_max_um, post_point.r_max_um)
        {
            return Err(format!(
                "cross-interaction axis differs at index {index}: [{}, {}) vs [{}, {})",
                pre_point.r_min_um, pre_point.r_max_um, post_point.r_min_um, post_point.r_max_um
            ));
        }
        if pre_point.value.is_some() != post_point.value.is_some() {
            return Err(format!(
                "cross-interaction availability differs at index {index}"
            ));
        }
    }
    Ok(())
}

fn axis_values_match(left: f64, right: f64) -> bool {
    let tolerance = AXIS_ABSOLUTE_TOLERANCE + AXIS_RELATIVE_TOLERANCE * left.abs().max(right.abs());
    (left - right).abs() <= tolerance
}
