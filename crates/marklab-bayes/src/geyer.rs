use serde::Serialize;

use crate::{
    strauss::{distance, validate_points},
    StraussError, StraussPoint,
};

#[derive(Debug, Serialize)]
pub struct GeyerPointSummary {
    pub point_id: String,
    pub neighbor_count: u32,
    pub saturated_count: u32,
}

#[derive(Debug, Serialize)]
pub struct GeyerSaturationResult {
    pub point_count: u32,
    pub interaction_radius_um: f64,
    pub saturation: u32,
    pub unordered_pair_visits: u64,
    pub interacting_pair_count: u64,
    pub statistic: u64,
    pub points: Vec<GeyerPointSummary>,
}

pub fn geyer_saturation_statistic(
    points: &[StraussPoint],
    radius: f64,
    saturation: u32,
    maximum_pair_visits: u64,
) -> Result<GeyerSaturationResult, StraussError> {
    validate_points(points)?;
    if !radius.is_finite() || radius <= 0.0 || !(1..=100_000_000).contains(&maximum_pair_visits) {
        return Err(StraussError::InvalidSpec(
            "Geyer radius or pair-visit cap is invalid".into(),
        ));
    }
    let n = points.len() as u64;
    let visits = n
        .checked_mul(n.saturating_sub(1))
        .and_then(|v| v.checked_div(2))
        .ok_or_else(|| StraussError::Resource("Geyer pair work overflows".into()))?;
    if visits > maximum_pair_visits {
        return Err(StraussError::Resource(
            "Geyer pair work exceeds declared cap".into(),
        ));
    }
    let mut counts = vec![0_u32; points.len()];
    let mut interacting = 0_u64;
    for i in 0..points.len() {
        for j in i + 1..points.len() {
            if distance(&points[i], &points[j])? <= radius {
                counts[i] = counts[i].checked_add(1).ok_or_else(|| {
                    StraussError::Resource("Geyer neighbor count overflows".into())
                })?;
                counts[j] = counts[j].checked_add(1).ok_or_else(|| {
                    StraussError::Resource("Geyer neighbor count overflows".into())
                })?;
                interacting += 1;
            }
        }
    }
    let summaries = points
        .iter()
        .zip(counts)
        .map(|(point, count)| GeyerPointSummary {
            point_id: point.point_id.clone(),
            neighbor_count: count,
            saturated_count: count.min(saturation),
        })
        .collect::<Vec<_>>();
    let statistic = summaries.iter().map(|p| u64::from(p.saturated_count)).sum();
    Ok(GeyerSaturationResult {
        point_count: points.len() as u32,
        interaction_radius_um: radius,
        saturation,
        unordered_pair_visits: visits,
        interacting_pair_count: interacting,
        statistic,
        points: summaries,
    })
}
