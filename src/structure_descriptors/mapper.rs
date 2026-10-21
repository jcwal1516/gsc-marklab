//! Mapper graph of cell features over a one-dimensional lens (Singh, Mémoli and Carlsson
//! 2007).
//!
//! * **Cover:** with lens range `[m, M]` split into `n` equal bins of width `b = (M − m) / n`,
//!   interval `j` is the closed interval centred on bin `j` with half-length `b / (2(1 − p))`,
//!   so consecutive intervals overlap by the fraction `p` of their length.
//! * **Clustering:** within each interval, cells are joined when the Euclidean distance between
//!   their feature vectors is at most the declared threshold; clusters are the connected
//!   components (single linkage cut at the threshold). Clusters are numbered by their first
//!   cell in input order.
//! * **Graph:** each cluster is a node; two nodes are joined when they share at least the
//!   declared number of cells.
//!
//! The graph depends on the lens, the feature scaling, `n`, `p` and the threshold, all of which
//! are part of the declared design. It is a descriptive summary without a stability or
//! significance assessment.

use serde::{Deserialize, Serialize};

use crate::errors::{MarklabError, Result};

/// Result format of [`mapper_graph_from_json`].
pub const MAPPER_GRAPH_FORMAT: &str = "marklab.mapper_graph";
const MAXIMUM_INPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: String,
    version: u32,
    lens_name: String,
    feature_names: Vec<String>,
    cells: Vec<Cell>,
    cover: Cover,
    distance_threshold: f64,
    minimum_shared_cells: usize,
    limits: Limits,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cell {
    id: String,
    lens: f64,
    features: Vec<f64>,
}

/// Declared interval cover of the lens.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Cover {
    pub intervals: usize,
    pub overlap_fraction: f64,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    maximum_cells: usize,
    maximum_distance_evaluations: u64,
}

/// One lens interval.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LensInterval {
    pub index: usize,
    pub lower: f64,
    pub upper: f64,
    pub cells: usize,
    pub clusters: usize,
}

/// One Mapper node: a cluster of cells within one interval.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MapperNode {
    pub id: String,
    pub interval: usize,
    pub members: Vec<String>,
}

/// Two nodes sharing cells.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MapperEdge {
    pub source: String,
    pub target: String,
    pub shared_cells: usize,
}

/// Mapper graph of one cell table.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MapperGraphResult {
    pub format: String,
    pub version: u32,
    pub lens_name: String,
    pub feature_names: Vec<String>,
    pub cells: usize,
    pub cover: Cover,
    pub distance_threshold: f64,
    pub minimum_shared_cells: usize,
    pub intervals: Vec<LensInterval>,
    pub nodes: Vec<MapperNode>,
    pub edges: Vec<MapperEdge>,
    /// Connected components of the Mapper graph.
    pub components: usize,
    pub claim_scope: String,
}

fn invalid(message: impl Into<String>) -> MarklabError {
    MarklabError::Validation(message.into())
}

/// Parse a `marklab.mapper_graph_request` version 1 and build the graph.
pub fn mapper_graph_from_json(input: &[u8]) -> Result<MapperGraphResult> {
    if input.len() > MAXIMUM_INPUT_BYTES {
        return Err(invalid("Mapper input exceeds 16 MiB"));
    }
    let request: Request = serde_json::from_slice(input)?;
    let dimensions = request.feature_names.len();
    let cover = request.cover;
    if request.format != "marklab.mapper_graph_request"
        || request.version != 1
        || request.lens_name.trim().is_empty()
        || !(1..=256).contains(&dimensions)
        || request
            .feature_names
            .iter()
            .any(|name| name.trim().is_empty())
        || !(2..=request.limits.maximum_cells).contains(&request.cells.len())
        || !(1..=1000).contains(&cover.intervals)
        || !(0.0..=0.95).contains(&cover.overlap_fraction)
        || !(request.distance_threshold.is_finite() && request.distance_threshold > 0.0)
        || request.minimum_shared_cells == 0
    {
        return Err(invalid(
            "Mapper format, features (1 to 256), cells, intervals (1 to 1000), overlap \
             (0 to 0.95), distance threshold or shared-cell minimum are invalid",
        ));
    }
    let mut ids = std::collections::HashSet::new();
    if !request.cells.iter().all(|cell| {
        !cell.id.trim().is_empty()
            && ids.insert(cell.id.as_str())
            && cell.lens.is_finite()
            && cell.features.len() == dimensions
            && cell.features.iter().all(|value| value.is_finite())
    }) {
        return Err(invalid(
            "cells need unique identifiers, a finite lens and one finite value per feature",
        ));
    }
    let lenses = request.cells.iter().map(|cell| cell.lens);
    let minimum = lenses.clone().fold(f64::INFINITY, f64::min);
    let maximum = lenses.fold(f64::NEG_INFINITY, f64::max);
    if maximum <= minimum {
        return Err(invalid("the lens is constant, so the cover is undefined"));
    }
    let bin = (maximum - minimum) / cover.intervals as f64;
    let half = bin / (2.0 * (1.0 - cover.overlap_fraction));
    let bounds = (0..cover.intervals)
        .map(|index| {
            let center = minimum + (index as f64 + 0.5) * bin;
            (center - half, center + half)
        })
        .collect::<Vec<_>>();
    let members = bounds
        .iter()
        .map(|&(lower, upper)| {
            (0..request.cells.len())
                .filter(|&cell| (lower..=upper).contains(&request.cells[cell].lens))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let work = members.iter().fold(0_u64, |total, cells| {
        let count = cells.len() as u64;
        total.saturating_add(count.saturating_mul(count.saturating_sub(1)) / 2)
    });
    if work > request.limits.maximum_distance_evaluations {
        return Err(invalid(format!(
            "Mapper clustering needs {work} distance evaluations, above the declared limit"
        )));
    }

    let threshold = request.distance_threshold;
    let mut intervals = Vec::with_capacity(cover.intervals);
    let mut nodes = Vec::new();
    let mut node_cells = Vec::<Vec<usize>>::new();
    for (index, cells) in members.iter().enumerate() {
        let mut parent = (0..cells.len()).collect::<Vec<_>>();
        for a in 0..cells.len() {
            for b in a + 1..cells.len() {
                let distance = request.cells[cells[a]]
                    .features
                    .iter()
                    .zip(&request.cells[cells[b]].features)
                    .map(|(x, y)| (x - y) * (x - y))
                    .sum::<f64>()
                    .sqrt();
                if distance <= threshold {
                    let (root_a, root_b) = (find(&mut parent, a), find(&mut parent, b));
                    parent[root_a.max(root_b)] = root_a.min(root_b);
                }
            }
        }
        // Roots are the smallest member position, so clusters come out in first-cell order.
        let mut clusters = Vec::<Vec<usize>>::new();
        let mut cluster_of_root = vec![usize::MAX; cells.len()];
        for (position, &cell) in cells.iter().enumerate() {
            let root = find(&mut parent, position);
            if cluster_of_root[root] == usize::MAX {
                cluster_of_root[root] = clusters.len();
                clusters.push(Vec::new());
            }
            clusters[cluster_of_root[root]].push(cell);
        }
        intervals.push(LensInterval {
            index,
            lower: bounds[index].0,
            upper: bounds[index].1,
            cells: cells.len(),
            clusters: clusters.len(),
        });
        for (cluster, cluster_cells) in clusters.into_iter().enumerate() {
            nodes.push(MapperNode {
                id: format!("interval{index}_cluster{cluster}"),
                interval: index,
                members: cluster_cells
                    .iter()
                    .map(|&cell| request.cells[cell].id.clone())
                    .collect(),
            });
            node_cells.push(cluster_cells);
        }
    }

    // Each cell lies in a few consecutive intervals, so shared cells are found per cell.
    let mut nodes_of_cell = vec![Vec::<usize>::new(); request.cells.len()];
    for (node, cells) in node_cells.iter().enumerate() {
        for &cell in cells {
            nodes_of_cell[cell].push(node);
        }
    }
    let mut shared = std::collections::BTreeMap::<(usize, usize), usize>::new();
    for list in &nodes_of_cell {
        for (position, &a) in list.iter().enumerate() {
            for &b in &list[position + 1..] {
                *shared.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
    }
    let mut parent = (0..nodes.len()).collect::<Vec<_>>();
    let mut edges = Vec::new();
    for (&(a, b), &count) in &shared {
        if count >= request.minimum_shared_cells {
            let (root_a, root_b) = (find(&mut parent, a), find(&mut parent, b));
            parent[root_a.max(root_b)] = root_a.min(root_b);
            edges.push(MapperEdge {
                source: nodes[a].id.clone(),
                target: nodes[b].id.clone(),
                shared_cells: count,
            });
        }
    }
    let components = (0..nodes.len())
        .filter(|&node| find(&mut parent, node) == node)
        .count();
    Ok(MapperGraphResult {
        format: MAPPER_GRAPH_FORMAT.into(),
        version: 1,
        lens_name: request.lens_name,
        feature_names: request.feature_names,
        cells: request.cells.len(),
        cover,
        distance_threshold: threshold,
        minimum_shared_cells: request.minimum_shared_cells,
        intervals,
        nodes,
        edges,
        components,
        claim_scope: "descriptive_mapper_graph".into(),
    })
}

fn find(parent: &mut [usize], mut node: usize) -> usize {
    while parent[node] != node {
        parent[node] = parent[parent[node]];
        node = parent[node];
    }
    node
}
