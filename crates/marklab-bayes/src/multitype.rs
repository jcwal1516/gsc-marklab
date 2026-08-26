use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultitypePoint {
    pub point_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub type_id: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultitypeBaseline {
    pub type_id: String,
    pub log_baseline_per_um2: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultitypeInteraction {
    pub type_a: String,
    pub type_b: String,
    pub log_pair_potential: f64,
    pub radius_um: f64,
}
#[derive(Clone, Debug)]
pub struct MultitypePapangelouSpec {
    pub points: Vec<MultitypePoint>,
    pub baselines: Vec<MultitypeBaseline>,
    pub interactions: Vec<MultitypeInteraction>,
    pub proposal_type: String,
    pub proposal_x_um: f64,
    pub proposal_y_um: f64,
    pub maximum_visits: u64,
}
#[derive(Debug, Error)]
pub enum MultitypeError {
    #[error("invalid multitype Papangelou input: {0}")]
    Invalid(String),
    #[error("multitype Papangelou resource limit exceeded: {0}")]
    Resource(String),
    #[error("multitype Papangelou numerical failure: {0}")]
    Numerical(String),
}
#[derive(Debug, Serialize)]
pub struct MultitypeContribution {
    pub point_id: String,
    pub existing_type: String,
    pub distance_um: f64,
    pub radius_um: f64,
    pub included: bool,
    pub log_pair_potential: f64,
    pub applied_log_contribution: f64,
}
#[derive(Debug, Serialize)]
pub struct MultitypePapangelouResult {
    pub proposal_type: String,
    pub proposal_x_um: f64,
    pub proposal_y_um: f64,
    pub baseline_log_intensity: f64,
    pub pair_log_sum: f64,
    pub log_papangelou_per_um2: f64,
    pub papangelou_per_um2: f64,
    pub visits: u64,
    pub type_count: u32,
    pub point_count: u32,
    pub contributions: Vec<MultitypeContribution>,
}

pub fn multitype_papangelou(
    spec: MultitypePapangelouSpec,
) -> Result<MultitypePapangelouResult, MultitypeError> {
    if !(1..=64).contains(&spec.baselines.len())
        || spec.points.len() > 100_000
        || !(1..=100_000_000).contains(&spec.maximum_visits)
        || spec.points.len() as u64 > spec.maximum_visits
        || ![spec.proposal_x_um, spec.proposal_y_um]
            .into_iter()
            .all(f64::is_finite)
    {
        return Err(MultitypeError::Invalid(
            "type, point, proposal, or visit bounds are invalid".into(),
        ));
    }
    let mut baseline = BTreeMap::new();
    for b in &spec.baselines {
        if b.type_id.is_empty()
            || b.type_id.trim() != b.type_id
            || !b.log_baseline_per_um2.is_finite()
            || baseline
                .insert(b.type_id.as_str(), b.log_baseline_per_um2)
                .is_some()
        {
            return Err(MultitypeError::Invalid(
                "baselines require unique exact type IDs and finite values".into(),
            ));
        }
    }
    let base = *baseline
        .get(spec.proposal_type.as_str())
        .ok_or_else(|| MultitypeError::Invalid("proposal type has no baseline".into()))?;
    let expected = baseline.len() * baseline.len();
    if spec.interactions.len() != expected {
        return Err(MultitypeError::Invalid(
            "interaction matrix must contain every ordered type pair".into(),
        ));
    }
    let mut matrix = BTreeMap::new();
    for i in &spec.interactions {
        if !baseline.contains_key(i.type_a.as_str())
            || !baseline.contains_key(i.type_b.as_str())
            || !i.log_pair_potential.is_finite()
            || !i.radius_um.is_finite()
            || i.radius_um <= 0.0
            || matrix
                .insert(
                    (i.type_a.as_str(), i.type_b.as_str()),
                    (i.log_pair_potential, i.radius_um),
                )
                .is_some()
        {
            return Err(MultitypeError::Invalid(
                "interaction rows are invalid, repeated, or reference unknown types".into(),
            ));
        }
    }
    for ((a, b), (potential, radius)) in &matrix {
        let reverse = matrix
            .get(&(*b, *a))
            .ok_or_else(|| MultitypeError::Invalid("interaction matrix is incomplete".into()))?;
        if potential.to_bits() != reverse.0.to_bits() || radius.to_bits() != reverse.1.to_bits() {
            return Err(MultitypeError::Invalid(
                "interaction matrix and radii must be exactly symmetric".into(),
            ));
        }
    }
    let mut ids = HashSet::new();
    for p in &spec.points {
        if p.point_id.is_empty()
            || p.point_id.trim() != p.point_id
            || !ids.insert(p.point_id.as_str())
            || !baseline.contains_key(p.type_id.as_str())
            || ![p.x_um, p.y_um].into_iter().all(f64::is_finite)
            || (p.x_um == spec.proposal_x_um && p.y_um == spec.proposal_y_um)
        {
            return Err(MultitypeError::Invalid(
                "points require unique IDs, known types, finite coordinates, and distinct proposal"
                    .into(),
            ));
        }
    }
    let mut pair_sum = 0.0;
    let mut contributions = Vec::with_capacity(spec.points.len());
    for p in &spec.points {
        let distance = (spec.proposal_x_um - p.x_um).hypot(spec.proposal_y_um - p.y_um);
        if !distance.is_finite() {
            return Err(MultitypeError::Numerical("distance is non-finite".into()));
        }
        let (potential, radius) = matrix[&(spec.proposal_type.as_str(), p.type_id.as_str())];
        let included = distance <= radius;
        let applied = if included { potential } else { 0.0 };
        pair_sum += applied;
        contributions.push(MultitypeContribution {
            point_id: p.point_id.clone(),
            existing_type: p.type_id.clone(),
            distance_um: distance,
            radius_um: radius,
            included,
            log_pair_potential: potential,
            applied_log_contribution: applied,
        });
    }
    let log = base + pair_sum;
    let value = log.exp();
    if !log.is_finite() || !value.is_finite() {
        return Err(MultitypeError::Numerical(
            "conditional intensity overflows".into(),
        ));
    }
    Ok(MultitypePapangelouResult {
        proposal_type: spec.proposal_type,
        proposal_x_um: spec.proposal_x_um,
        proposal_y_um: spec.proposal_y_um,
        baseline_log_intensity: base,
        pair_log_sum: pair_sum,
        log_papangelou_per_um2: log,
        papangelou_per_um2: value,
        visits: spec.points.len() as u64,
        type_count: baseline.len() as u32,
        point_count: spec.points.len() as u32,
        contributions,
    })
}
