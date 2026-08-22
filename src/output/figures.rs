use std::path::Path;

use crate::errors::{MarklabError, Result};

use super::MarkedPatternResult;

pub(super) fn render_spectrum_svg(result: &MarkedPatternResult) -> String {
    let spectrum = result
        .spectrum
        .value()
        .expect("spectrum figure requires available spectrum");
    let points = &result.spectrum_curve;
    let k_values = points.iter().map(|point| point.k).collect::<Vec<_>>();
    let power_values = points
        .iter()
        .map(|point| point.whitened_power)
        .collect::<Vec<_>>();
    let (k_min, k_max) = finite_range(&k_values);
    let (power_min, power_max) = finite_range(&power_values);
    let polyline = points
        .iter()
        .map(|point| {
            let x = scale(point.k, k_min, k_max, 44.0, 300.0);
            let y = scale(point.whitened_power, power_min, power_max, 126.0, 28.0);
            format!("{x:.2},{y:.2}")
        })
        .collect::<Vec<_>>()
        .join(" ");

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 340 160"><title>marklab spectrum</title><rect width="340" height="160" fill="white"/><text x="14" y="22">low-k excess {low_k:.3}</text><line x1="44" y1="128" x2="306" y2="128" stroke="#333"/><line x1="44" y1="24" x2="44" y2="128" stroke="#333"/><polyline points="{polyline}" fill="none" stroke="#1f77b4" stroke-width="2"/><text x="244" y="148">k</text><text x="8" y="36">S white</text></svg>"##,
        low_k = spectrum.low_k_excess,
        polyline = polyline
    )
}

pub(super) fn render_anisotropy_svg(result: &MarkedPatternResult) -> String {
    let anisotropy = result
        .anisotropy
        .value()
        .expect("anisotropy figure requires available anisotropy");
    let length = (42.0 * anisotropy.index.clamp(1.0, 3.0) / 3.0).max(16.0);
    let direction = anisotropy.theta_deg.map_or_else(
        || r#"<text x="105" y="84">orientation undefined</text>"#.to_owned(),
        |theta_deg| {
            let theta = theta_deg.to_radians();
            let x2 = 170.0 + length * theta.cos();
            let y2 = 80.0 - length * theta.sin();
            format!(
                r##"<line x1="170" y1="80" x2="{x2:.2}" y2="{y2:.2}" stroke="#d62728" stroke-width="3"/>"##
            )
        },
    );
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 340 160"><title>marklab anisotropy</title><rect width="340" height="160" fill="white"/><circle cx="170" cy="80" r="46" fill="none" stroke="#777"/>{direction}<text class="anisotropy-index" x="14" y="24">anisotropy-index {index:.3}</text><text x="14" y="44">theta-deg {theta_deg}</text><text x="14" y="64">p-value {p_value}</text></svg>"##,
        direction = direction,
        index = anisotropy.index,
        theta_deg = optional_f64(anisotropy.theta_deg),
        p_value = optional_f64(anisotropy.p_value)
    )
}

pub(super) fn render_scale_energy_svg(result: &MarkedPatternResult) -> String {
    let points = &result.scale_energy_curve;
    let max_energy = points
        .iter()
        .map(|point| point.energy_fraction)
        .filter(|value| value.is_finite())
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let mut bars = String::new();
    for (index, point) in points.iter().enumerate() {
        let x = 46.0 + index as f64 * 92.0;
        let height = (point.energy_fraction.max(0.0) / max_energy * 88.0).min(88.0);
        let y = 124.0 - height;
        bars.push_str(&format!(
            r##"<rect x="{x:.2}" y="{y:.2}" width="46" height="{height:.2}" fill="#2ca02c"/><text x="{x:.2}" y="144">{band}</text>"##,
            band = xml_text(point.band.as_str())
        ));
    }

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 340 160"><title>marklab scale energy</title><rect width="340" height="160" fill="white"/><text x="14" y="22">relative scale energy</text><line x1="36" y1="126" x2="316" y2="126" stroke="#333"/>{bars}</svg>"##,
        bars = bars
    )
}

pub(super) fn render_territory_overlay_svg(result: &MarkedPatternResult) -> String {
    let territories = result
        .residual_territories
        .value()
        .expect("territory figure requires available territories");
    let mut circles = String::new();
    let max_radius = territories
        .iter()
        .map(|territory| territory.radius_um)
        .filter(|value| value.is_finite())
        .fold(0.0_f64, f64::max)
        .max(1.0);
    for territory in territories {
        let x = scale(
            territory.center_x_um,
            0.0,
            result.window.l_eff_um.max(1.0),
            40.0,
            300.0,
        );
        let y = scale(
            territory.center_y_um,
            0.0,
            result.window.l_eff_um.max(1.0),
            124.0,
            30.0,
        );
        let radius = (territory.radius_um / max_radius * 24.0).clamp(4.0, 30.0);
        circles.push_str(&format!(
            r##"<circle cx="{x:.2}" cy="{y:.2}" r="{radius:.2}" fill="none" stroke="#9467bd" stroke-width="2"/>"##
        ));
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 340 160"><title>marklab territory overlay</title><rect width="340" height="160" fill="white"/><text x="14" y="24">candidate territories: {count}</text>{circles}</svg>"##,
        count = territories.len(),
        circles = circles
    )
}

fn finite_range(values: &[f64]) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for value in values.iter().copied().filter(|value| value.is_finite()) {
        min = min.min(value);
        max = max.max(value);
    }
    if !min.is_finite() || !max.is_finite() {
        return (0.0, 1.0);
    }
    if (max - min).abs() < f64::EPSILON {
        (min, min + 1.0)
    } else {
        (min, max)
    }
}

fn scale(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    if !value.is_finite() || (from_max - from_min).abs() < f64::EPSILON {
        return (to_min + to_max) * 0.5;
    }
    let t = ((value - from_min) / (from_max - from_min)).clamp(0.0, 1.0);
    to_min + t * (to_max - to_min)
}

fn optional_f64(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "null".into())
}

fn xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
pub(super) fn write(result: &MarkedPatternResult, out: &Path) -> Result<()> {
    let figures = out.join("figures");
    std::fs::create_dir_all(&figures).map_err(|source| MarklabError::io(&figures, source))?;

    if !result.spectrum_curve.is_empty() {
        let spectrum_path = figures.join("spectrum.svg");
        std::fs::write(&spectrum_path, render_spectrum_svg(result))
            .map_err(|source| MarklabError::io(&spectrum_path, source))?;
    }

    if result.anisotropy.value().is_some() {
        let anisotropy_path = figures.join("anisotropy.svg");
        std::fs::write(&anisotropy_path, render_anisotropy_svg(result))
            .map_err(|source| MarklabError::io(&anisotropy_path, source))?;
    }

    if !result.scale_energy_curve.is_empty() {
        let scale_energy_path = figures.join("scale_energy.svg");
        std::fs::write(&scale_energy_path, render_scale_energy_svg(result))
            .map_err(|source| MarklabError::io(&scale_energy_path, source))?;
    }

    if result
        .residual_territories
        .value()
        .is_some_and(|territories| !territories.is_empty())
    {
        let territory_path = figures.join("residual_territory_overlay.svg");
        std::fs::write(&territory_path, render_territory_overlay_svg(result))
            .map_err(|source| MarklabError::io(&territory_path, source))?;
    }
    Ok(())
}
