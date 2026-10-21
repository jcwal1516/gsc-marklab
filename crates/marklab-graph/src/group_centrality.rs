use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{sparse_radius_graph::build_sparse_radius_graph, GraphError, GraphNodeInput};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphGroupCentralityNode {
    pub id: String,
    pub coordinates_um: [f64; 2],
    pub group_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphGroupCentralitySpec {
    pub nodes: Vec<GraphGroupCentralityNode>,
    pub radius_um: f64,
    pub maximum_nodes: usize,
    pub maximum_candidate_pairs: u64,
    pub maximum_edges: u64,
    pub maximum_group_distance_work: u64,
    pub maximum_clustering_wedges: u64,
    pub maximum_working_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphGroupCentralityRow {
    pub group_id: String,
    pub node_count: usize,
    pub external_neighbor_count: usize,
    pub degree_centrality: f64,
    pub closeness_centrality: f64,
    pub average_clustering: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphGroupCentralityResult {
    pub format: String,
    pub version: u32,
    pub node_count: usize,
    pub edge_count: u64,
    pub component_count: usize,
    pub group_count: usize,
    pub radius_um: f64,
    pub groups: Vec<GraphGroupCentralityRow>,
    pub candidate_pair_evaluations: u64,
    pub group_distance_work: u64,
    pub clustering_wedges: u64,
    pub working_bytes: u64,
}

pub fn graph_group_centrality_workflow(
    mut spec: GraphGroupCentralitySpec,
) -> Result<GraphGroupCentralityResult, GraphError> {
    validate_controls(&spec)?;
    if spec.nodes.iter().any(|node| {
        node.group_id.is_empty()
            || node.group_id.trim() != node.group_id
            || node.group_id.chars().any(char::is_control)
    }) {
        return Err(GraphError::Invalid(
            "group centrality nodes require non-empty exact group IDs without control characters"
                .into(),
        ));
    }
    let text_bytes = spec.nodes.iter().try_fold(0_u64, |total, node| {
        total
            .checked_add(node.id.len() as u64)?
            .checked_add(node.group_id.len() as u64)
    });
    let text_bytes = text_bytes
        .ok_or_else(|| GraphError::Invalid("group centrality text byte count overflow".into()))?;
    let preflight_nodes = u64::try_from(spec.nodes.len())
        .map_err(|_| GraphError::Invalid("group centrality node count overflow".into()))?;
    let maximum_possible_edges = preflight_nodes
        .checked_mul(preflight_nodes.saturating_sub(1))
        .map(|value| value / 2)
        .ok_or_else(|| GraphError::Invalid("group centrality edge count overflow".into()))?;
    let preflight_working_bytes = conservative_working_bytes_from_counts(
        preflight_nodes,
        spec.maximum_edges.min(maximum_possible_edges),
        preflight_nodes,
        preflight_nodes,
        text_bytes,
    )?;
    if preflight_working_bytes > spec.maximum_working_bytes {
        return Err(GraphError::Invalid(
            "group centrality working bytes exceed caller maximum".into(),
        ));
    }
    spec.nodes.sort_by(|left, right| left.id.cmp(&right.id));

    let mut labels = Vec::with_capacity(spec.nodes.len());
    let mut graph_nodes = Vec::with_capacity(spec.nodes.len());
    for node in std::mem::take(&mut spec.nodes) {
        labels.push(node.group_id);
        graph_nodes.push(GraphNodeInput {
            id: node.id,
            coordinates_um: node.coordinates_um,
            signal: 0.0,
        });
    }
    let graph = build_sparse_radius_graph(
        graph_nodes,
        spec.radius_um,
        spec.maximum_nodes,
        spec.maximum_candidate_pairs,
        spec.maximum_edges,
    )?;
    let node_count = graph.nodes.len();
    let edge_count = u64::try_from(graph.edges.len())
        .map_err(|_| GraphError::Invalid("group centrality edge count overflow".into()))?;

    let mut members_by_group = BTreeMap::<String, Vec<usize>>::new();
    for (index, group_id) in labels.into_iter().enumerate() {
        members_by_group.entry(group_id).or_default().push(index);
    }
    if members_by_group.len() < 2 {
        return Err(GraphError::Invalid(
            "group centrality requires at least two non-empty groups".into(),
        ));
    }

    let mut adjacency = graph
        .degrees
        .iter()
        .map(|degree| Vec::with_capacity(*degree as usize))
        .collect::<Vec<_>>();
    for &(left, right) in &graph.edges {
        adjacency[left].push(right);
        adjacency[right].push(left);
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable();
    }

    let component_count = count_components(&adjacency)?;
    let clustering_wedges = graph.degrees.iter().try_fold(0_u64, |total, degree| {
        let degree = u64::from(*degree);
        total.checked_add(degree.saturating_sub(1).saturating_mul(degree) / 2)
    });
    let clustering_wedges = clustering_wedges
        .ok_or_else(|| GraphError::Invalid("group centrality wedge count overflow".into()))?;
    if clustering_wedges > spec.maximum_clustering_wedges {
        return Err(GraphError::Invalid(
            "group centrality clustering wedges exceed caller maximum".into(),
        ));
    }

    let traversal_per_group = u64::try_from(node_count)
        .ok()
        .and_then(|nodes| edge_count.checked_mul(2)?.checked_add(nodes))
        .ok_or_else(|| GraphError::Invalid("group distance work overflow".into()))?;
    let planned_distance_work = traversal_per_group
        .checked_mul(members_by_group.len() as u64)
        .ok_or_else(|| GraphError::Invalid("group distance work overflow".into()))?;
    if planned_distance_work > spec.maximum_group_distance_work {
        return Err(GraphError::Invalid(
            "group distance work exceeds caller maximum".into(),
        ));
    }

    let working_bytes = conservative_working_bytes(
        node_count,
        graph.edges.len(),
        graph.cell_count,
        members_by_group.len(),
        text_bytes,
    )?;
    if working_bytes > spec.maximum_working_bytes {
        return Err(GraphError::Invalid(
            "group centrality working bytes exceed caller maximum".into(),
        ));
    }

    let local_clustering = local_clustering(&adjacency)?;
    let mut group_distance_work = 0_u64;
    let mut groups = Vec::with_capacity(members_by_group.len());
    for (group_id, members) in members_by_group {
        groups.push(group_row(
            group_id,
            &members,
            &adjacency,
            &local_clustering,
            &mut group_distance_work,
        )?);
    }
    if group_distance_work > spec.maximum_group_distance_work
        || groups.iter().any(|row| {
            [
                row.degree_centrality,
                row.closeness_centrality,
                row.average_clustering,
            ]
            .iter()
            .any(|value| !value.is_finite())
        })
    {
        return Err(GraphError::Numerical(
            "group centrality produced an invalid finite result".into(),
        ));
    }

    Ok(GraphGroupCentralityResult {
        format: "marklab.graph_group_centrality".into(),
        version: 1,
        node_count,
        edge_count,
        component_count,
        group_count: groups.len(),
        radius_um: spec.radius_um,
        groups,
        candidate_pair_evaluations: graph.candidate_pair_evaluations,
        group_distance_work,
        clustering_wedges,
        working_bytes,
    })
}

fn validate_controls(spec: &GraphGroupCentralitySpec) -> Result<(), GraphError> {
    if spec.nodes.len() < 2
        || spec.maximum_nodes < 2
        || spec.nodes.len() > spec.maximum_nodes
        || !spec.radius_um.is_finite()
        || spec.radius_um <= 0.0
        || !(spec.radius_um * spec.radius_um).is_finite()
        || spec.maximum_candidate_pairs == 0
        || spec.maximum_edges == 0
        || spec.maximum_group_distance_work == 0
        || spec.maximum_clustering_wedges == 0
        || spec.maximum_working_bytes == 0
    {
        return Err(GraphError::Invalid(
            "group centrality controls or resource limits are invalid".into(),
        ));
    }
    Ok(())
}

fn count_components(adjacency: &[Vec<usize>]) -> Result<usize, GraphError> {
    let mut visited = vec![false; adjacency.len()];
    let mut queue = VecDeque::new();
    let mut components = 0_usize;
    for start in 0..adjacency.len() {
        if visited[start] {
            continue;
        }
        components = components
            .checked_add(1)
            .ok_or_else(|| GraphError::Invalid("component count overflow".into()))?;
        visited[start] = true;
        queue.push_back(start);
        while let Some(node) = queue.pop_front() {
            for &neighbor in &adjacency[node] {
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
    }
    Ok(components)
}

fn local_clustering(adjacency: &[Vec<usize>]) -> Result<Vec<f64>, GraphError> {
    adjacency
        .iter()
        .map(|neighbors| {
            if neighbors.len() < 2 {
                return Ok(0.0);
            }
            let mut triangles = 0_u64;
            for (position, &left) in neighbors.iter().enumerate() {
                for &right in &neighbors[(position + 1)..] {
                    if adjacency[left].binary_search(&right).is_ok() {
                        triangles = triangles.checked_add(1).ok_or_else(|| {
                            GraphError::Invalid("group centrality triangle count overflow".into())
                        })?;
                    }
                }
            }
            let degree = neighbors.len() as f64;
            Ok(2.0 * triangles as f64 / (degree * (degree - 1.0)))
        })
        .collect()
}

fn group_row(
    group_id: String,
    members: &[usize],
    adjacency: &[Vec<usize>],
    local_clustering: &[f64],
    work: &mut u64,
) -> Result<GraphGroupCentralityRow, GraphError> {
    let node_count = adjacency.len();
    let outside_count = node_count
        .checked_sub(members.len())
        .ok_or_else(|| GraphError::Invalid("group size exceeds graph size".into()))?;
    if outside_count == 0 {
        return Err(GraphError::Invalid(
            "group centrality does not admit an all-node group".into(),
        ));
    }

    let mut distances = vec![usize::MAX; node_count];
    let mut external = vec![false; node_count];
    let mut queue = VecDeque::with_capacity(node_count);
    for &member in members {
        distances[member] = 0;
        queue.push_back(member);
    }
    while let Some(node) = queue.pop_front() {
        *work = work
            .checked_add(1)
            .ok_or_else(|| GraphError::Invalid("group distance work overflow".into()))?;
        let next_distance = distances[node]
            .checked_add(1)
            .ok_or_else(|| GraphError::Invalid("group graph distance overflow".into()))?;
        for &neighbor in &adjacency[node] {
            *work = work
                .checked_add(1)
                .ok_or_else(|| GraphError::Invalid("group distance work overflow".into()))?;
            if distances[neighbor] == usize::MAX {
                distances[neighbor] = next_distance;
                queue.push_back(neighbor);
            }
        }
    }

    for &member in members {
        for &neighbor in &adjacency[member] {
            if distances[neighbor] != 0 {
                external[neighbor] = true;
            }
        }
    }
    let external_neighbor_count = external.iter().filter(|value| **value).count();
    let distance_sum = distances
        .iter()
        .filter(|distance| **distance != 0 && **distance != usize::MAX)
        .try_fold(0_u64, |total, distance| {
            total.checked_add(u64::try_from(*distance).ok()?)
        })
        .ok_or_else(|| GraphError::Invalid("group distance sum overflow".into()))?;
    let degree_centrality = external_neighbor_count as f64 / outside_count as f64;
    let closeness_centrality = if distance_sum == 0 {
        0.0
    } else {
        outside_count as f64 / distance_sum as f64
    };
    let average_clustering = members
        .iter()
        .map(|member| local_clustering[*member])
        .sum::<f64>()
        / members.len() as f64;
    Ok(GraphGroupCentralityRow {
        group_id,
        node_count: members.len(),
        external_neighbor_count,
        degree_centrality,
        closeness_centrality,
        average_clustering,
    })
}

fn conservative_working_bytes(
    nodes: usize,
    edges: usize,
    cells: usize,
    groups: usize,
    text_bytes: u64,
) -> Result<u64, GraphError> {
    let nodes = u64::try_from(nodes)
        .map_err(|_| GraphError::Invalid("group centrality node count overflow".into()))?;
    let edges = u64::try_from(edges)
        .map_err(|_| GraphError::Invalid("group centrality edge count overflow".into()))?;
    let cells = u64::try_from(cells)
        .map_err(|_| GraphError::Invalid("group centrality cell count overflow".into()))?;
    let groups = u64::try_from(groups)
        .map_err(|_| GraphError::Invalid("group centrality group count overflow".into()))?;
    conservative_working_bytes_from_counts(nodes, edges, cells, groups, text_bytes)
}

fn conservative_working_bytes_from_counts(
    nodes: u64,
    edges: u64,
    cells: u64,
    groups: u64,
    text_bytes: u64,
) -> Result<u64, GraphError> {
    nodes
        .checked_mul(768)
        .and_then(|value| value.checked_add(edges.checked_mul(96)?))
        .and_then(|value| value.checked_add(cells.checked_mul(128)?))
        .and_then(|value| value.checked_add(groups.checked_mul(512)?))
        .and_then(|value| value.checked_add(text_bytes.checked_mul(2)?))
        .ok_or_else(|| GraphError::Invalid("group centrality working byte count overflow".into()))
}
