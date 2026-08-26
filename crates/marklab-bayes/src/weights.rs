use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SymmetryPolicy {
    Required,
    NotRequired,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagonalPolicy {
    Zero,
    Allow,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationPolicy {
    Preserve,
    RowStandardize,
}

#[derive(Clone, Copy, Debug)]
pub struct SpatialWeightsPolicy {
    pub symmetry: SymmetryPolicy,
    pub diagonal: DiagonalPolicy,
    pub normalization: NormalizationPolicy,
}

#[derive(Clone, Debug)]
pub struct SpatialEdge {
    pub source_region: String,
    pub target_region: String,
    pub weight: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ValidatedWeight {
    pub source_index: usize,
    pub target_index: usize,
    pub weight: f64,
}

#[derive(Clone, Debug)]
pub struct ValidatedSpatialWeights {
    pub region_ids: Vec<String>,
    pub weights: Vec<ValidatedWeight>,
    pub policy: SpatialWeightsPolicy,
    pub components: Vec<Vec<usize>>,
    pub islands: Vec<usize>,
    pub digest_sha256: String,
}

impl ValidatedSpatialWeights {
    pub fn row_sums(&self) -> Vec<f64> {
        let mut sums = vec![0.0; self.region_ids.len()];
        for edge in &self.weights {
            sums[edge.source_index] += edge.weight;
        }
        sums
    }
}

#[derive(Debug, Error)]
pub enum SpatialWeightsError {
    #[error("invalid spatial weights: {0}")]
    InvalidInput(String),
}

pub fn validate_spatial_weights(
    mut region_ids: Vec<String>,
    edges: Vec<SpatialEdge>,
    policy: SpatialWeightsPolicy,
) -> Result<ValidatedSpatialWeights, SpatialWeightsError> {
    if region_ids.is_empty() || region_ids.len() > 100_000 {
        return Err(SpatialWeightsError::InvalidInput(
            "region count must be between 1 and 100000".into(),
        ));
    }
    if edges.len() > 2_000_000 {
        return Err(SpatialWeightsError::InvalidInput(
            "edge count exceeds 2000000".into(),
        ));
    }
    region_ids.sort();
    for (index, region_id) in region_ids.iter().enumerate() {
        if region_id.is_empty()
            || region_id.trim() != region_id
            || (index > 0 && region_ids[index - 1] == *region_id)
        {
            return Err(SpatialWeightsError::InvalidInput(
                "region IDs must be exact, non-empty, and unique".into(),
            ));
        }
    }
    let region_index = region_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut raw = BTreeMap::<(usize, usize), f64>::new();
    for edge in edges {
        let source = *region_index
            .get(edge.source_region.as_str())
            .ok_or_else(|| {
                SpatialWeightsError::InvalidInput(format!(
                    "unknown source region {}",
                    edge.source_region
                ))
            })?;
        let target = *region_index
            .get(edge.target_region.as_str())
            .ok_or_else(|| {
                SpatialWeightsError::InvalidInput(format!(
                    "unknown target region {}",
                    edge.target_region
                ))
            })?;
        if !edge.weight.is_finite() || edge.weight <= 0.0 {
            return Err(SpatialWeightsError::InvalidInput(
                "stored weights must be finite and positive".into(),
            ));
        }
        if source == target && matches!(policy.diagonal, DiagonalPolicy::Zero) {
            return Err(SpatialWeightsError::InvalidInput(
                "self-weight violates zero-diagonal policy".into(),
            ));
        }
        if raw.insert((source, target), edge.weight).is_some() {
            return Err(SpatialWeightsError::InvalidInput(
                "duplicate directed edge".into(),
            ));
        }
    }
    if matches!(policy.symmetry, SymmetryPolicy::Required) {
        for (&(source, target), &weight) in &raw {
            if source == target {
                continue;
            }
            let reciprocal = raw.get(&(target, source)).ok_or_else(|| {
                SpatialWeightsError::InvalidInput("required reciprocal edge is missing".into())
            })?;
            if reciprocal.to_bits() != weight.to_bits() {
                return Err(SpatialWeightsError::InvalidInput(
                    "reciprocal weights differ under required symmetry".into(),
                ));
            }
        }
    }
    let mut row_sums = vec![0.0; region_ids.len()];
    for (&(source, _), &weight) in &raw {
        row_sums[source] += weight;
    }
    let weights = raw
        .iter()
        .map(|(&(source_index, target_index), &weight)| ValidatedWeight {
            source_index,
            target_index,
            weight: match policy.normalization {
                NormalizationPolicy::Preserve => weight,
                NormalizationPolicy::RowStandardize => weight / row_sums[source_index],
            },
        })
        .collect::<Vec<_>>();
    if weights.iter().any(|edge| !edge.weight.is_finite()) {
        return Err(SpatialWeightsError::InvalidInput(
            "normalization produced a non-finite weight".into(),
        ));
    }
    let mut adjacency = vec![BTreeSet::new(); region_ids.len()];
    for edge in &weights {
        if edge.source_index != edge.target_index {
            adjacency[edge.source_index].insert(edge.target_index);
            adjacency[edge.target_index].insert(edge.source_index);
        }
    }
    let islands = adjacency
        .iter()
        .enumerate()
        .filter_map(|(index, neighbors)| neighbors.is_empty().then_some(index))
        .collect::<Vec<_>>();
    let mut visited = vec![false; region_ids.len()];
    let mut components = Vec::new();
    for start in 0..region_ids.len() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut queue = VecDeque::from([start]);
        let mut component = Vec::new();
        while let Some(current) = queue.pop_front() {
            component.push(current);
            for &neighbor in &adjacency[current] {
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        components.push(component);
    }
    let digest_sha256 = digest(&region_ids, &weights, policy);
    Ok(ValidatedSpatialWeights {
        region_ids,
        weights,
        policy,
        components,
        islands,
        digest_sha256,
    })
}

fn digest(
    region_ids: &[String],
    weights: &[ValidatedWeight],
    policy: SpatialWeightsPolicy,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"marklab-validated-spatial-weights-v1\0");
    hasher.update([
        policy.symmetry as u8,
        policy.diagonal as u8,
        policy.normalization as u8,
    ]);
    hasher.update((region_ids.len() as u64).to_be_bytes());
    for id in region_ids {
        hasher.update((id.len() as u64).to_be_bytes());
        hasher.update(id.as_bytes());
    }
    hasher.update((weights.len() as u64).to_be_bytes());
    for edge in weights {
        hasher.update((edge.source_index as u64).to_be_bytes());
        hasher.update((edge.target_index as u64).to_be_bytes());
        hasher.update(edge.weight.to_bits().to_be_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standardized_chain_and_island_are_explicit() {
        let weights = validate_spatial_weights(
            vec!["d".into(), "b".into(), "a".into(), "c".into()],
            vec![
                edge("a", "b", 2.0),
                edge("b", "a", 2.0),
                edge("b", "c", 1.0),
                edge("c", "b", 1.0),
            ],
            SpatialWeightsPolicy {
                symmetry: SymmetryPolicy::Required,
                diagonal: DiagonalPolicy::Zero,
                normalization: NormalizationPolicy::RowStandardize,
            },
        )
        .expect("weights");
        assert_eq!(weights.components.len(), 2);
        assert_eq!(weights.islands, vec![3]);
        assert_eq!(weights.row_sums(), vec![1.0, 1.0, 1.0, 0.0]);
    }

    fn edge(source: &str, target: &str, weight: f64) -> SpatialEdge {
        SpatialEdge {
            source_region: source.into(),
            target_region: target.into(),
            weight,
        }
    }
}
