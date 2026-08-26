use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::GraphError;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HeterogeneousNodeInput {
    pub id: String,
    pub node_type: String,
    pub coordinates_um: [f64; 2],
    pub features: Vec<f64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageAggregation {
    Sum,
    Mean,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialNearRelationSpec {
    pub name: String,
    pub source_type: String,
    pub target_type: String,
    pub radius_um: f64,
    pub message_scale: f64,
    pub aggregation: MessageAggregation,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeterogeneousMessageSpec {
    pub nodes: Vec<HeterogeneousNodeInput>,
    pub relations: Vec<SpatialNearRelationSpec>,
    pub maximum_pair_evaluations: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct HeterogeneousEdge {
    pub relation: String,
    pub source_id: String,
    pub target_id: String,
    pub distance_um: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct HeterogeneousMessageResult {
    pub format: &'static str,
    pub version: u32,
    pub graph_digest: String,
    pub nodes: Vec<HeterogeneousNodeInput>,
    pub relations: Vec<SpatialNearRelationSpec>,
    pub edges: Vec<HeterogeneousEdge>,
    pub updated_features: BTreeMap<String, Vec<f64>>,
    pub pair_evaluations: u64,
    pub message_evaluations: u64,
    pub claim_status: &'static str,
}

pub fn heterogeneous_graph_message_workflow(
    mut spec: HeterogeneousMessageSpec,
) -> Result<HeterogeneousMessageResult, GraphError> {
    validate(&spec)?;
    spec.nodes.sort_by(|left, right| left.id.cmp(&right.id));
    spec.relations
        .sort_by(|left, right| left.name.cmp(&right.name));
    let index = spec
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let types = spec.nodes.iter().enumerate().fold(
        BTreeMap::<&str, Vec<usize>>::new(),
        |mut map, (index, node)| {
            map.entry(node.node_type.as_str()).or_default().push(index);
            map
        },
    );
    let mut pair_evaluations = 0_u64;
    let mut edges = Vec::new();
    for relation in &spec.relations {
        let sources = types.get(relation.source_type.as_str()).ok_or_else(|| {
            GraphError::Invalid(format!(
                "relation {} has unknown source type {}",
                relation.name, relation.source_type
            ))
        })?;
        let targets = types.get(relation.target_type.as_str()).ok_or_else(|| {
            GraphError::Invalid(format!(
                "relation {} has unknown target type {}",
                relation.name, relation.target_type
            ))
        })?;
        for &source in sources {
            for &target in targets {
                if source == target {
                    continue;
                }
                pair_evaluations = pair_evaluations.checked_add(1).ok_or_else(|| {
                    GraphError::Invalid("heterogeneous pair work overflow".into())
                })?;
                if pair_evaluations > spec.maximum_pair_evaluations {
                    return Err(GraphError::Invalid(format!(
                        "heterogeneous pair work exceeds caller maximum {}",
                        spec.maximum_pair_evaluations
                    )));
                }
                let distance = spec.nodes[source]
                    .coordinates_um
                    .iter()
                    .zip(spec.nodes[target].coordinates_um)
                    .map(|(left, right)| (left - right).powi(2))
                    .sum::<f64>()
                    .sqrt();
                if distance <= relation.radius_um {
                    edges.push(HeterogeneousEdge {
                        relation: relation.name.clone(),
                        source_id: spec.nodes[source].id.clone(),
                        target_id: spec.nodes[target].id.clone(),
                        distance_um: distance,
                    });
                }
            }
        }
    }
    edges.sort_by(|left, right| {
        (&left.relation, &left.source_id, &left.target_id).cmp(&(
            &right.relation,
            &right.source_id,
            &right.target_id,
        ))
    });
    let feature_count = spec.nodes[0].features.len();
    let mut message_sums = vec![vec![0.0; feature_count]; spec.nodes.len()];
    let mut message_counts = vec![0_usize; spec.nodes.len()];
    for edge in &edges {
        let source = index[edge.source_id.as_str()];
        let target = index[edge.target_id.as_str()];
        let relation = spec
            .relations
            .iter()
            .find(|relation| relation.name == edge.relation)
            .expect("validated relation edge");
        for (sum, value) in message_sums[target]
            .iter_mut()
            .zip(&spec.nodes[source].features)
        {
            *sum += relation.message_scale * value;
        }
        message_counts[target] += 1;
    }
    let mut updated_features = BTreeMap::new();
    for (node_index, node) in spec.nodes.iter().enumerate() {
        let mut updated = node.features.clone();
        if message_counts[node_index] > 0 {
            let incoming_relations = edges
                .iter()
                .filter(|edge| edge.target_id == node.id)
                .map(|edge| edge.relation.as_str())
                .collect::<HashSet<_>>();
            if incoming_relations.len() != 1 {
                return Err(GraphError::Invalid(
                    "version-one message passing permits one incoming relation per target node"
                        .into(),
                ));
            }
            let relation = spec
                .relations
                .iter()
                .find(|relation| incoming_relations.contains(relation.name.as_str()))
                .expect("incoming relation");
            let denominator = match relation.aggregation {
                MessageAggregation::Sum => 1.0,
                MessageAggregation::Mean => message_counts[node_index] as f64,
            };
            for (value, message) in updated.iter_mut().zip(&message_sums[node_index]) {
                *value += message / denominator;
            }
        }
        if updated.iter().any(|value| !value.is_finite()) {
            return Err(GraphError::Numerical(
                "heterogeneous message update is non-finite".into(),
            ));
        }
        updated_features.insert(node.id.clone(), updated);
    }
    let digest = serde_json::to_vec(&serde_json::json!({
        "nodes": &spec.nodes,
        "relations": &spec.relations,
        "edges": &edges,
    }))
    .map_err(|error| GraphError::Numerical(error.to_string()))?;
    Ok(HeterogeneousMessageResult {
        format: "marklab.heterogeneous_graph_message",
        version: 1,
        graph_digest: format!("{:x}", Sha256::digest(digest)),
        nodes: spec.nodes,
        relations: spec.relations,
        edges,
        updated_features,
        pair_evaluations,
        message_evaluations: message_counts.iter().sum::<usize>() as u64,
        claim_status: "descriptive_typed_message_passing_only",
    })
}

fn validate(spec: &HeterogeneousMessageSpec) -> Result<(), GraphError> {
    if !(2..=100_000).contains(&spec.nodes.len())
        || spec.relations.is_empty()
        || spec.relations.len() > 1_024
        || spec.maximum_pair_evaluations == 0
    {
        return Err(GraphError::Invalid(
            "heterogeneous graph requires bounded nodes, relations, and pair work".into(),
        ));
    }
    let feature_count = spec.nodes[0].features.len();
    let mut node_ids = HashSet::new();
    if feature_count == 0
        || spec.nodes.iter().any(|node| {
            node.id.is_empty()
                || node.id.trim() != node.id
                || !node_ids.insert(node.id.as_str())
                || node.node_type.is_empty()
                || node.node_type.trim() != node.node_type
                || node.coordinates_um.iter().any(|value| !value.is_finite())
                || node.features.len() != feature_count
                || node.features.iter().any(|value| !value.is_finite())
        })
    {
        return Err(GraphError::Invalid(
            "heterogeneous nodes require unique exact IDs/types and aligned finite features".into(),
        ));
    }
    let mut relation_names = HashSet::new();
    if spec.relations.iter().any(|relation| {
        relation.name.is_empty()
            || relation.name.trim() != relation.name
            || !relation_names.insert(relation.name.as_str())
            || relation.source_type.is_empty()
            || relation.target_type.is_empty()
            || !relation.radius_um.is_finite()
            || relation.radius_um <= 0.0
            || !relation.message_scale.is_finite()
    }) {
        return Err(GraphError::Invalid(
            "relations require unique exact names/types and finite positive radii".into(),
        ));
    }
    Ok(())
}
