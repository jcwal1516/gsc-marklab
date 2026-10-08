use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StraussPoint {
    pub point_id: String,
    pub x_um: f64,
    pub y_um: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct StraussProposal {
    pub x_um: f64,
    pub y_um: f64,
}

#[derive(Debug, Error)]
pub enum StraussError {
    #[error("invalid Strauss specification: {0}")]
    InvalidSpec(String),
    #[error("Strauss resource limit exceeded: {0}")]
    Resource(String),
    #[error("Strauss numerical failure: {0}")]
    Numerical(String),
}

#[derive(Debug, Serialize)]
pub struct StraussSufficientStatistics {
    pub point_count: u64,
    pub unordered_pair_visits: u64,
    pub interacting_pair_count: u64,
}

#[derive(Debug, Serialize)]
pub struct StraussPapangelou {
    pub proposal: StraussProposal,
    pub beta_per_um2: f64,
    pub gamma: f64,
    pub neighbor_delta: u64,
    pub papangelou_per_um2: f64,
}

pub fn strauss_statistics(
    points: &[StraussPoint],
    interaction_radius_um: f64,
    maximum_pair_visits: u64,
) -> Result<StraussSufficientStatistics, StraussError> {
    validate_points(points)?;
    validate_radius(interaction_radius_um)?;
    if !(1..=100_000_000).contains(&maximum_pair_visits) {
        return Err(StraussError::InvalidSpec(
            "maximum pair visits must be in 1-100000000".into(),
        ));
    }
    let point_count = points.len() as u64;
    let pair_visits = point_count
        .checked_mul(point_count.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or_else(|| StraussError::Resource("pair visit count overflows".into()))?;
    if pair_visits > maximum_pair_visits {
        return Err(StraussError::Resource(format!(
            "required {pair_visits} pair visits exceed declared cap {maximum_pair_visits}"
        )));
    }
    let mut interacting = 0_u64;
    for left in 0..points.len() {
        for right in left + 1..points.len() {
            if distance(&points[left], &points[right])? <= interaction_radius_um {
                interacting += 1;
            }
        }
    }
    Ok(StraussSufficientStatistics {
        point_count,
        unordered_pair_visits: pair_visits,
        interacting_pair_count: interacting,
    })
}

pub fn strauss_papangelou(
    points: &[StraussPoint],
    proposal: StraussProposal,
    beta_per_um2: f64,
    gamma: f64,
    interaction_radius_um: f64,
) -> Result<StraussPapangelou, StraussError> {
    validate_points(points)?;
    validate_radius(interaction_radius_um)?;
    if ![proposal.x_um, proposal.y_um, beta_per_um2, gamma]
        .into_iter()
        .all(f64::is_finite)
        || beta_per_um2 <= 0.0
        || !(0.0..=1.0).contains(&gamma)
        || points
            .iter()
            .any(|point| point.x_um == proposal.x_um && point.y_um == proposal.y_um)
    {
        return Err(StraussError::InvalidSpec(
            "proposal, positive beta, gamma in [0,1], or simple-pattern insertion is invalid"
                .into(),
        ));
    }
    let coordinates = points
        .iter()
        .map(|point| (point.x_um, point.y_um))
        .collect::<Vec<_>>();
    let neighbor_delta = neighbor_delta(
        (proposal.x_um, proposal.y_um),
        &coordinates,
        interaction_radius_um,
    )?;
    let papangelou = papangelou_from_delta(beta_per_um2, gamma, neighbor_delta);
    if !papangelou.is_finite() {
        return Err(StraussError::Numerical(
            "Papangelou intensity is non-finite".into(),
        ));
    }
    Ok(StraussPapangelou {
        proposal,
        beta_per_um2,
        gamma,
        neighbor_delta,
        papangelou_per_um2: papangelou,
    })
}

pub(crate) fn neighbor_delta(
    proposal: (f64, f64),
    points: &[(f64, f64)],
    interaction_radius_um: f64,
) -> Result<u64, StraussError> {
    let mut count = 0_u64;
    for point in points {
        let distance = (proposal.0 - point.0).hypot(proposal.1 - point.1);
        if !distance.is_finite() {
            return Err(StraussError::Numerical(
                "Strauss proposal distance is non-finite".into(),
            ));
        }
        if distance <= interaction_radius_um {
            count += 1;
        }
    }
    Ok(count)
}

pub(crate) fn papangelou_from_delta(beta_per_um2: f64, gamma: f64, delta: u64) -> f64 {
    if gamma == 0.0 {
        if delta == 0 {
            beta_per_um2
        } else {
            0.0
        }
    } else {
        beta_per_um2 * gamma.powi(delta as i32)
    }
}

pub(crate) fn validate_points(points: &[StraussPoint]) -> Result<(), StraussError> {
    if points.len() > 100_000 {
        return Err(StraussError::InvalidSpec(
            "Strauss pattern exceeds 100000 points".into(),
        ));
    }
    let mut ids = std::collections::HashSet::with_capacity(points.len());
    for point in points {
        if point.point_id.is_empty()
            || point.point_id.trim() != point.point_id
            || !point.x_um.is_finite()
            || !point.y_um.is_finite()
            || !ids.insert(point.point_id.as_str())
        {
            return Err(StraussError::InvalidSpec(
                "Strauss points require unique exact IDs and finite coordinates".into(),
            ));
        }
    }
    Ok(())
}

fn validate_radius(radius: f64) -> Result<(), StraussError> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(StraussError::InvalidSpec(
            "Strauss interaction radius must be finite and positive".into(),
        ));
    }
    Ok(())
}

pub(crate) fn distance(left: &StraussPoint, right: &StraussPoint) -> Result<f64, StraussError> {
    let value = (left.x_um - right.x_um).hypot(left.y_um - right.y_um);
    if !value.is_finite() {
        return Err(StraussError::Numerical(
            "Strauss pair distance is non-finite".into(),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_gamma_obeys_zero_neighbor_convention() {
        let points = vec![StraussPoint {
            point_id: "a".into(),
            x_um: 0.0,
            y_um: 0.0,
        }];
        let isolated = strauss_papangelou(
            &points,
            StraussProposal {
                x_um: 10.0,
                y_um: 0.0,
            },
            2.0,
            0.0,
            1.0,
        )
        .unwrap();
        let interacting = strauss_papangelou(
            &points,
            StraussProposal {
                x_um: 0.5,
                y_um: 0.0,
            },
            2.0,
            0.0,
            1.0,
        )
        .unwrap();
        assert_eq!(isolated.papangelou_per_um2, 2.0);
        assert_eq!(interacting.papangelou_per_um2, 0.0);
    }
}
