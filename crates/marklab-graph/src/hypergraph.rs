use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::GraphError;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HypergraphNodeInput {
    pub id: String,
    pub signal: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HypergraphMemberInput {
    pub node_id: String,
    pub membership: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HyperedgeInput {
    pub id: String,
    pub hyperedge_type: String,
    pub weight: f64,
    pub members: Vec<HypergraphMemberInput>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HypergraphSignalSpec {
    pub nodes: Vec<HypergraphNodeInput>,
    pub hyperedges: Vec<HyperedgeInput>,
    pub epsilon: f64,
    pub maximum_incidence_entries: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct IncidenceEntry {
    pub node_id: String,
    pub hyperedge_id: String,
    pub membership: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct HypergraphSignalResult {
    pub format: &'static str,
    pub version: u32,
    pub hypergraph_digest: String,
    pub nodes: Vec<HypergraphNodeInput>,
    pub hyperedges: Vec<HyperedgeInput>,
    pub incidence: Vec<IncidenceEntry>,
    pub incidence_entries: u64,
    pub node_degrees: Vec<f64>,
    pub hyperedge_degrees: Vec<f64>,
    pub laplacian: Vec<Vec<f64>>,
    pub signal_numerator: f64,
    pub signal_denominator: f64,
    pub signal_smoothness: f64,
    pub claim_status: &'static str,
}

pub fn hypergraph_signal_workflow(
    mut spec: HypergraphSignalSpec,
) -> Result<HypergraphSignalResult, GraphError> {
    validate(&spec)?;
    spec.nodes.sort_by(|left, right| left.id.cmp(&right.id));
    spec.hyperedges
        .sort_by(|left, right| left.id.cmp(&right.id));
    for hyperedge in &mut spec.hyperedges {
        hyperedge
            .members
            .sort_by(|left, right| left.node_id.cmp(&right.node_id));
    }
    let node_index = spec
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let incidence_entries = spec
        .hyperedges
        .iter()
        .map(|edge| edge.members.len() as u64)
        .sum::<u64>();
    if incidence_entries > spec.maximum_incidence_entries {
        return Err(GraphError::Invalid(format!(
            "incidence entries {incidence_entries} exceed caller maximum {}",
            spec.maximum_incidence_entries
        )));
    }
    let node_count = spec.nodes.len();
    let edge_count = spec.hyperedges.len();
    let mut incidence_matrix = vec![vec![0.0; edge_count]; node_count];
    let mut incidence = Vec::with_capacity(incidence_entries as usize);
    for (edge_index, edge) in spec.hyperedges.iter().enumerate() {
        for member in &edge.members {
            let node = node_index
                .get(member.node_id.as_str())
                .copied()
                .ok_or_else(|| {
                    GraphError::Invalid(format!(
                        "hyperedge {} has unknown node {}",
                        edge.id, member.node_id
                    ))
                })?;
            incidence_matrix[node][edge_index] = member.membership;
            incidence.push(IncidenceEntry {
                node_id: member.node_id.clone(),
                hyperedge_id: edge.id.clone(),
                membership: member.membership,
            });
        }
    }
    let hyperedge_degrees = (0..edge_count)
        .map(|edge| incidence_matrix.iter().map(|row| row[edge]).sum::<f64>())
        .collect::<Vec<_>>();
    let node_degrees = (0..node_count)
        .map(|node| {
            spec.hyperedges
                .iter()
                .enumerate()
                .map(|(edge, definition)| definition.weight * incidence_matrix[node][edge])
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    if node_degrees.iter().any(|degree| *degree <= 0.0)
        || hyperedge_degrees.iter().any(|degree| *degree <= 0.0)
    {
        return Err(GraphError::Invalid(
            "hypergraph contains an isolated node or empty hyperedge".into(),
        ));
    }
    let mut laplacian = vec![vec![0.0; node_count]; node_count];
    for left in 0..node_count {
        for right in 0..node_count {
            let theta = (0..edge_count)
                .map(|edge| {
                    incidence_matrix[left][edge]
                        * spec.hyperedges[edge].weight
                        * incidence_matrix[right][edge]
                        / hyperedge_degrees[edge]
                        / (node_degrees[left] * node_degrees[right]).sqrt()
                })
                .sum::<f64>();
            laplacian[left][right] = f64::from(left == right) - theta;
        }
    }
    let signal = spec
        .nodes
        .iter()
        .map(|node| node.signal)
        .collect::<Vec<_>>();
    let signal_numerator = signal
        .iter()
        .enumerate()
        .map(|(left, value)| {
            value
                * laplacian[left]
                    .iter()
                    .zip(&signal)
                    .map(|(operator, right)| operator * right)
                    .sum::<f64>()
        })
        .sum::<f64>();
    let signal_denominator = signal
        .iter()
        .map(|value| value.powi(2))
        .sum::<f64>()
        .max(spec.epsilon);
    let signal_smoothness = signal_numerator / signal_denominator;
    if laplacian.iter().flatten().any(|value| !value.is_finite()) || !signal_smoothness.is_finite()
    {
        return Err(GraphError::Numerical(
            "hypergraph operator or smoothness is non-finite".into(),
        ));
    }
    let digest = serde_json::to_vec(&serde_json::json!({
        "nodes": &spec.nodes,
        "hyperedges": &spec.hyperedges,
        "incidence": &incidence,
    }))
    .map_err(|error| GraphError::Numerical(error.to_string()))?;
    Ok(HypergraphSignalResult {
        format: "marklab.hypergraph_signal",
        version: 1,
        hypergraph_digest: format!("{:x}", Sha256::digest(digest)),
        nodes: spec.nodes,
        hyperedges: spec.hyperedges,
        incidence,
        incidence_entries,
        node_degrees,
        hyperedge_degrees,
        laplacian,
        signal_numerator,
        signal_denominator,
        signal_smoothness,
        claim_status: "experimental_declared_hypergraph_signal",
    })
}

fn validate(spec: &HypergraphSignalSpec) -> Result<(), GraphError> {
    if !(2..=10_000).contains(&spec.nodes.len())
        || spec.hyperedges.is_empty()
        || spec.hyperedges.len() > 10_000
        || !spec.epsilon.is_finite()
        || spec.epsilon <= 0.0
        || spec.maximum_incidence_entries == 0
    {
        return Err(GraphError::Invalid(
            "hypergraph dimensions, epsilon, or incidence limit are invalid".into(),
        ));
    }
    let mut node_ids = HashSet::new();
    if spec.nodes.iter().any(|node| {
        node.id.is_empty()
            || node.id.trim() != node.id
            || !node_ids.insert(node.id.as_str())
            || !node.signal.is_finite()
    }) {
        return Err(GraphError::Invalid(
            "hypergraph nodes require unique exact IDs and finite signals".into(),
        ));
    }
    let mut edge_ids = HashSet::new();
    for edge in &spec.hyperedges {
        if edge.id.is_empty()
            || edge.id.trim() != edge.id
            || !edge_ids.insert(edge.id.as_str())
            || edge.hyperedge_type.is_empty()
            || edge.hyperedge_type.trim() != edge.hyperedge_type
            || !edge.weight.is_finite()
            || edge.weight <= 0.0
            || edge.members.len() < 2
        {
            return Err(GraphError::Invalid(
                "hyperedges require unique IDs/types, positive weight, and at least two members"
                    .into(),
            ));
        }
        let mut members = HashSet::new();
        if edge.members.iter().any(|member| {
            member.node_id.is_empty()
                || !members.insert(member.node_id.as_str())
                || !member.membership.is_finite()
                || member.membership <= 0.0
        }) {
            return Err(GraphError::Invalid(
                "hyperedge members require unique node IDs and positive finite membership".into(),
            ));
        }
    }
    Ok(())
}
