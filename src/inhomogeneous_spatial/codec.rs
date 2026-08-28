use std::io;

use serde::{de::DeserializeOwned, Serialize};

use crate::{ObservationWindow2D, Pattern};

use super::{
    identity::{fixed_grid_digest, intensity_result_digest},
    types::{InhomogeneousIntensitySummary, InhomogeneousSpatialConfig},
};

pub(super) fn encode_exact_float_json<T: Serialize>(value: &T) -> io::Result<Box<[u8]>> {
    let mut value = serde_json::to_value(value).map_err(invalid_owned)?;
    encode_float_bits(&mut value);
    serde_json::to_vec_pretty(&value)
        .map(Vec::into_boxed_slice)
        .map_err(invalid_owned)
}

pub(super) fn decode_exact_float_json<T: DeserializeOwned>(bytes: &[u8]) -> io::Result<T> {
    let mut value: serde_json::Value = serde_json::from_slice(bytes).map_err(invalid_owned)?;
    decode_float_bits(&mut value)?;
    serde_json::from_value(value).map_err(invalid_owned)
}

pub(super) fn validate_intensity_summary(
    summary: &InhomogeneousIntensitySummary,
    pattern: &Pattern,
    window: &ObservationWindow2D,
    config: &InhomogeneousSpatialConfig,
) -> io::Result<()> {
    if summary.estimator != "gaussian_kernel"
        || summary.kernel != "isotropic_gaussian_2d"
        || summary.cross_fit != "leave_one_out_n_over_n_minus_one"
        || summary.boundary_correction != "deterministic_cell_center_quadrature"
        || summary.bandwidth_um.to_bits() != config.bandwidth_um().to_bits()
        || summary.integration_grid != config.integration_grid()
        || summary.minimum_intensity_per_um2.to_bits()
            != config.minimum_intensity_per_um2().to_bits()
        || summary.point_values.len() != pattern.len()
        || summary.retained_probe_count != summary.fixed_grid.len()
    {
        return Err(invalid("intensity summary does not match its request"));
    }
    let expected_digest = intensity_result_digest(
        pattern,
        window,
        config,
        &summary.point_values,
        &summary.fixed_grid,
    );
    if summary.artifact_digest != expected_digest.to_string() {
        return Err(invalid("intensity artifact digest is inconsistent"));
    }
    validate_fixed_grid(summary, window, config)?;
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for (row, point) in summary.point_values.iter().enumerate() {
        if point.row != row
            || point.training_point_count != pattern.len() - 1
            || !point.intensity_per_um2.is_finite()
            || point.intensity_per_um2 < config.minimum_intensity_per_um2()
            || !point.boundary_mass.is_finite()
            || point.boundary_mass <= 0.0
        {
            return Err(invalid("intensity row is inconsistent or non-finite"));
        }
        minimum = minimum.min(point.intensity_per_um2);
        maximum = maximum.max(point.intensity_per_um2);
    }
    if minimum.to_bits() != summary.observed_minimum_intensity_per_um2.to_bits()
        || maximum.to_bits() != summary.observed_maximum_intensity_per_um2.to_bits()
    {
        return Err(invalid("intensity extrema are inconsistent"));
    }
    Ok(())
}

fn validate_fixed_grid(
    summary: &InhomogeneousIntensitySummary,
    window: &ObservationWindow2D,
    config: &InhomogeneousSpatialConfig,
) -> io::Result<()> {
    if summary.fixed_grid_digest
        != fixed_grid_digest(window, config, &summary.fixed_grid).to_string()
    {
        return Err(invalid("fixed intensity grid digest is inconsistent"));
    }
    let [min_x, min_y, max_x, max_y] = window.bounds_um();
    let grid = config.integration_grid();
    let spacing = [
        (max_x - min_x) / grid[0] as f64,
        (max_y - min_y) / grid[1] as f64,
    ];
    if summary.probe_spacing_um[0].to_bits() != spacing[0].to_bits()
        || summary.probe_spacing_um[1].to_bits() != spacing[1].to_bits()
        || summary.maximum_probe_displacement_um.to_bits()
            != (0.5 * spacing[0].hypot(spacing[1])).to_bits()
    {
        return Err(invalid("fixed intensity grid spacing is inconsistent"));
    }
    let cell_area = spacing[0] * spacing[1];
    let mut retained = 0_usize;
    let mut total = 0.0;
    let mut correction = 0.0;
    for y in 0..grid[1] {
        for x in 0..grid[0] {
            let center_x = min_x + (x as f64 + 0.5) * spacing[0];
            let center_y = min_y + (y as f64 + 0.5) * spacing[1];
            if !window.contains(center_x, center_y) {
                continue;
            }
            let point = summary
                .fixed_grid
                .get(retained)
                .ok_or_else(|| invalid("fixed intensity grid is truncated"))?;
            let expected_mass = point.intensity_per_um2 * cell_area;
            if point.probe_index != retained
                || point.x_um.to_bits() != center_x.to_bits()
                || point.y_um.to_bits() != center_y.to_bits()
                || !point.intensity_per_um2.is_finite()
                || point.intensity_per_um2 <= 0.0
                || !point.cell_mass.is_finite()
                || point.cell_mass <= 0.0
                || point.cell_mass.to_bits().abs_diff(expected_mass.to_bits()) > 1
            {
                return Err(invalid("fixed intensity grid row is inconsistent"));
            }
            compensated_add(&mut total, &mut correction, point.cell_mass);
            retained = retained
                .checked_add(1)
                .ok_or_else(|| invalid("fixed intensity grid size overflow"))?;
        }
    }
    let total = total + correction;
    if retained != summary.fixed_grid.len()
        || total
            .to_bits()
            .abs_diff(summary.fixed_grid_total_mass.to_bits())
            > 1
    {
        return Err(invalid("fixed intensity grid total is inconsistent"));
    }
    Ok(())
}

const FLOAT_BITS_KEY: &str = "__marklab_f64_bits";

fn encode_float_bits(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Number(number) if number.is_f64() => {
            let bits = number.as_f64().expect("f64 JSON number").to_bits();
            *value = serde_json::json!({ (FLOAT_BITS_KEY): bits });
        }
        serde_json::Value::Array(values) => {
            for value in values {
                encode_float_bits(value);
            }
        }
        serde_json::Value::Object(fields) => {
            for value in fields.values_mut() {
                encode_float_bits(value);
            }
        }
        _ => {}
    }
}

fn decode_float_bits(value: &mut serde_json::Value) -> io::Result<()> {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                decode_float_bits(value)?;
            }
        }
        serde_json::Value::Object(fields)
            if fields.len() == 1 && fields.contains_key(FLOAT_BITS_KEY) =>
        {
            let bits = fields[FLOAT_BITS_KEY]
                .as_u64()
                .ok_or_else(|| invalid("exact f64 bit tag is invalid"))?;
            let number = serde_json::Number::from_f64(f64::from_bits(bits))
                .ok_or_else(|| invalid("exact f64 bit tag is non-finite"))?;
            *value = serde_json::Value::Number(number);
        }
        serde_json::Value::Object(fields) => {
            for value in fields.values_mut() {
                decode_float_bits(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn compensated_add(sum: &mut f64, correction: &mut f64, value: f64) {
    let corrected = value - *correction;
    let next = *sum + corrected;
    *correction = (next - *sum) - corrected;
    *sum = next;
}

pub(super) fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
