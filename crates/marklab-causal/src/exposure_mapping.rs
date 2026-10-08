use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::{Deserialize, Serialize};

use crate::CausalError;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExposureMappingUnit {
    pub unit_id: String,
    pub treatment: bool,
    pub continuous_field_value: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExposureGraphEdge {
    pub left_unit: String,
    pub right_unit: String,
    pub weight: f64,
    pub distance_um: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExposureMappingKind {
    BinaryAnyTreated,
    CountTreated,
    WeightedFractionTreated,
    GaussianDistanceDecay,
    MultiscaleCountTreated,
    ContinuousField,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExposureMappingSpec {
    pub graph_provenance: String,
    pub units: Vec<ExposureMappingUnit>,
    pub edges: Vec<ExposureGraphEdge>,
    pub kind: ExposureMappingKind,
    pub bandwidth_um: Option<f64>,
    pub radii_um: Vec<f64>,
    pub maximum_directed_edge_visits: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct UnitExposureValue {
    pub unit_id: String,
    pub treatment: bool,
    pub scalar_value: Option<f64>,
    pub vector_value: Option<Vec<f64>>,
    pub status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExposureMappingResult {
    pub format: &'static str,
    pub version: u32,
    pub graph_provenance: String,
    pub kind: ExposureMappingKind,
    pub bandwidth_um: Option<f64>,
    pub radii_um: Vec<f64>,
    pub exposures: Vec<UnitExposureValue>,
    pub directed_edge_visits: u64,
    pub claim_status: &'static str,
}

struct Neighbor {
    index: usize,
    weight: f64,
    distance_um: f64,
}

pub fn compute_spatial_exposure_mapping(
    mut spec: ExposureMappingSpec,
) -> Result<ExposureMappingResult, CausalError> {
    validate_settings(&spec)?;
    spec.units
        .sort_by(|left, right| left.unit_id.cmp(&right.unit_id));
    let indices = spec
        .units
        .iter()
        .enumerate()
        .map(|(index, unit)| (unit.unit_id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let adjacency = compile_graph(&spec, &indices)?;
    let base_visits = u64::try_from(spec.edges.len())
        .ok()
        .and_then(|edges| edges.checked_mul(2))
        .ok_or_else(|| CausalError::Resource("directed edge visits overflowed".into()))?;
    let visits = match spec.kind {
        ExposureMappingKind::ContinuousField => 0,
        ExposureMappingKind::MultiscaleCountTreated => base_visits
            .checked_mul(spec.radii_um.len() as u64)
            .ok_or_else(|| CausalError::Resource("multiscale edge visits overflowed".into()))?,
        _ => base_visits,
    };
    if visits > spec.maximum_directed_edge_visits {
        return Err(CausalError::Resource(format!(
            "directed edge visits {visits} exceed declared maximum {}",
            spec.maximum_directed_edge_visits
        )));
    }
    let exposures = spec
        .units
        .iter()
        .enumerate()
        .map(|(unit_index, unit)| map_unit(unit, &spec, &adjacency[unit_index]))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ExposureMappingResult {
        format: "marklab.spatial_exposure_mapping",
        version: 1,
        graph_provenance: spec.graph_provenance,
        kind: spec.kind,
        bandwidth_um: spec.bandwidth_um,
        radii_um: spec.radii_um,
        exposures,
        directed_edge_visits: visits,
        claim_status: "exposure_construction_only",
    })
}

fn map_unit(
    unit: &ExposureMappingUnit,
    spec: &ExposureMappingSpec,
    neighbors: &[Neighbor],
) -> Result<UnitExposureValue, CausalError> {
    let (scalar_value, vector_value, status) = match spec.kind {
        ExposureMappingKind::BinaryAnyTreated => (
            Some(f64::from(
                neighbors
                    .iter()
                    .any(|neighbor| spec.units[neighbor.index].treatment),
            )),
            None,
            "available",
        ),
        ExposureMappingKind::CountTreated => (
            Some(
                neighbors
                    .iter()
                    .filter(|neighbor| spec.units[neighbor.index].treatment)
                    .count() as f64,
            ),
            None,
            "available",
        ),
        ExposureMappingKind::WeightedFractionTreated => {
            let denominator = neighbors
                .iter()
                .map(|neighbor| neighbor.weight)
                .sum::<f64>();
            if denominator == 0.0 {
                (None, None, "unavailable_isolated_unit")
            } else {
                let numerator = neighbors
                    .iter()
                    .filter(|neighbor| spec.units[neighbor.index].treatment)
                    .map(|neighbor| neighbor.weight)
                    .sum::<f64>();
                (Some(numerator / denominator), None, "available")
            }
        }
        ExposureMappingKind::GaussianDistanceDecay => {
            let bandwidth = spec.bandwidth_um.expect("validated bandwidth");
            let value = neighbors
                .iter()
                .filter(|neighbor| spec.units[neighbor.index].treatment)
                .map(|neighbor| (-0.5 * (neighbor.distance_um / bandwidth).powi(2)).exp())
                .sum::<f64>();
            (Some(value), None, "available")
        }
        ExposureMappingKind::MultiscaleCountTreated => {
            let values = spec
                .radii_um
                .iter()
                .map(|radius| {
                    neighbors
                        .iter()
                        .filter(|neighbor| {
                            neighbor.distance_um <= *radius && spec.units[neighbor.index].treatment
                        })
                        .count() as f64
                })
                .collect();
            (None, Some(values), "available")
        }
        ExposureMappingKind::ContinuousField => (
            unit.continuous_field_value,
            None,
            "available_declared_field",
        ),
    };
    if scalar_value.is_some_and(|value| !value.is_finite())
        || vector_value
            .as_ref()
            .is_some_and(|values: &Vec<f64>| values.iter().any(|value| !value.is_finite()))
    {
        return Err(CausalError::Numerical(format!(
            "exposure for unit {} is not finite",
            unit.unit_id
        )));
    }
    Ok(UnitExposureValue {
        unit_id: unit.unit_id.clone(),
        treatment: unit.treatment,
        scalar_value,
        vector_value,
        status,
    })
}

fn validate_settings(spec: &ExposureMappingSpec) -> Result<(), CausalError> {
    if spec.graph_provenance.trim().is_empty() || spec.units.is_empty() {
        return Err(CausalError::Invalid(
            "graph provenance and at least one unit are required".into(),
        ));
    }
    let mut ids = HashSet::new();
    for unit in &spec.units {
        if unit.unit_id.trim().is_empty()
            || !ids.insert(unit.unit_id.as_str())
            || unit
                .continuous_field_value
                .is_some_and(|value| !value.is_finite())
        {
            return Err(CausalError::Invalid(
                "unit IDs must be unique/nonempty and field values finite".into(),
            ));
        }
    }
    let uses_bandwidth = matches!(spec.kind, ExposureMappingKind::GaussianDistanceDecay);
    let valid_bandwidth = spec
        .bandwidth_um
        .is_some_and(|value| value.is_finite() && value > 0.0);
    if uses_bandwidth != valid_bandwidth {
        return Err(CausalError::Invalid(
            "only Gaussian distance decay requires one positive finite bandwidth".into(),
        ));
    }
    let uses_radii = matches!(spec.kind, ExposureMappingKind::MultiscaleCountTreated);
    let valid_radii = !spec.radii_um.is_empty()
        && spec
            .radii_um
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0)
        && spec.radii_um.windows(2).all(|pair| pair[0] < pair[1]);
    if uses_radii != valid_radii {
        return Err(CausalError::Invalid(
            "only multiscale mapping requires nonempty increasing nonnegative radii".into(),
        ));
    }
    if matches!(spec.kind, ExposureMappingKind::ContinuousField)
        && spec
            .units
            .iter()
            .any(|unit| unit.continuous_field_value.is_none())
    {
        return Err(CausalError::Invalid(
            "continuous-field mapping requires a value for every unit".into(),
        ));
    }
    Ok(())
}

fn compile_graph(
    spec: &ExposureMappingSpec,
    indices: &BTreeMap<String, usize>,
) -> Result<Vec<Vec<Neighbor>>, CausalError> {
    let mut adjacency = (0..spec.units.len())
        .map(|_| Vec::new())
        .collect::<Vec<_>>();
    let mut unique = BTreeSet::new();
    for edge in &spec.edges {
        let left = indices.get(edge.left_unit.trim()).copied().ok_or_else(|| {
            CausalError::Invalid(format!("edge references missing unit {}", edge.left_unit))
        })?;
        let right = indices
            .get(edge.right_unit.trim())
            .copied()
            .ok_or_else(|| {
                CausalError::Invalid(format!("edge references missing unit {}", edge.right_unit))
            })?;
        let key = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        if left == right
            || !unique.insert(key)
            || !edge.weight.is_finite()
            || edge.weight <= 0.0
            || !edge.distance_um.is_finite()
            || edge.distance_um <= 0.0
        {
            return Err(CausalError::Invalid(
                "edges must be unique/non-self with positive finite weight and distance".into(),
            ));
        }
        adjacency[left].push(Neighbor {
            index: right,
            weight: edge.weight,
            distance_um: edge.distance_um,
        });
        adjacency[right].push(Neighbor {
            index: left,
            weight: edge.weight,
            distance_um: edge.distance_um,
        });
    }
    for neighbors in &mut adjacency {
        neighbors.sort_by_key(|neighbor| neighbor.index);
    }
    Ok(adjacency)
}
