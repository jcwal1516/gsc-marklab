/// Diameter of a circle with `area_um2`.
pub(crate) fn equivalent_area_diameter_um(area_um2: f64) -> Option<f64> {
    if !area_um2.is_finite() || area_um2 <= 0.0 {
        return None;
    }
    let diameter = (4.0 * area_um2 / std::f64::consts::PI).sqrt();
    diameter.is_finite().then_some(diameter)
}

/// Diagonal of the axis-aligned point bounding box.
pub(crate) fn bounding_box_diagonal_um(x_um: &[f64], y_um: &[f64]) -> Option<f64> {
    if x_um.len() != y_um.len()
        || x_um.len() < 2
        || x_um
            .iter()
            .chain(y_um.iter())
            .any(|coordinate| !coordinate.is_finite())
    {
        return None;
    }
    let min_x = x_um.iter().copied().reduce(f64::min)?;
    let max_x = x_um.iter().copied().reduce(f64::max)?;
    let min_y = y_um.iter().copied().reduce(f64::min)?;
    let max_y = y_um.iter().copied().reduce(f64::max)?;
    let diagonal = (max_x - min_x).hypot(max_y - min_y);
    (diagonal.is_finite() && diagonal > 0.0).then_some(diagonal)
}

/// Analysis length recorded on the window, with an explicit point-bounding-box
/// fallback for programmatically constructed patterns that have no mask area.
pub(crate) fn analysis_effective_length_um(
    recorded_length_um: f64,
    x_um: &[f64],
    y_um: &[f64],
) -> Option<f64> {
    if recorded_length_um.is_finite() && recorded_length_um > 0.0 {
        Some(recorded_length_um)
    } else {
        bounding_box_diagonal_um(x_um, y_um)
    }
}

/// Largest physical scale eligible for inference.
pub(crate) fn maximum_interpretable_scale_um(
    analysis_effective_length_um: f64,
    fraction: f64,
) -> Option<f64> {
    if !analysis_effective_length_um.is_finite()
        || analysis_effective_length_um <= 0.0
        || !fraction.is_finite()
        || !(0.0..1.0).contains(&fraction)
    {
        return None;
    }
    let scale = analysis_effective_length_um * fraction;
    (scale.is_finite() && scale > 0.0).then_some(scale)
}

pub(crate) fn maximum_interpretable_scale_for_points_um(
    recorded_length_um: f64,
    x_um: &[f64],
    y_um: &[f64],
    fraction: f64,
) -> Option<f64> {
    maximum_interpretable_scale_um(
        analysis_effective_length_um(recorded_length_um, x_um, y_um)?,
        fraction,
    )
}
