use std::{collections::BTreeMap, mem::size_of};

use crate::{
    errors::{MarklabError, Result},
    geom::spatial_index::{Neighbor, SpatialIndex2D},
    multimodal::cells::FusedCell,
    perf::counters::enforce_storage_budget,
};

#[cfg(test)]
thread_local! {
    static GRAPH_BUILD_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_graph_build_call_count() {
    GRAPH_BUILD_CALLS.set(0);
}

#[cfg(test)]
pub(crate) fn graph_build_call_count() -> usize {
    GRAPH_BUILD_CALLS.get()
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// Configuration for spatial neighborhood graph construction.
///
/// `radius_um` creates undirected edges for all cell pairs whose registered
/// coordinate distance is less than or equal to the radius. `k_nearest` adds
/// the union of each node's per-node nearest-neighbor choices, with ties broken
/// by lower target index. When both options are set, the graph contains the
/// union of both edge sets.
pub struct GraphConfig {
    /// Inclusive maximum pair distance, in microns, for radius edges.
    pub radius_um: Option<f64>,
    /// Number of nearest neighbors selected independently for each node.
    pub k_nearest: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
/// A single undirected spatial edge stored once as a sorted node-index pair.
pub struct SpatialEdge {
    /// Lower endpoint index into the input cell slice.
    pub source: usize,
    /// Higher endpoint index into the input cell slice.
    pub target: usize,
    /// Euclidean distance between endpoint registered coordinates, in microns.
    pub distance_um: f64,
    /// Angle from `source` to `target` in radians.
    pub angle_rad: f64,
    /// True when `distance_um` is strictly less than
    /// `2 * max(endpoint registration_error_um)`.
    ///
    /// Missing endpoint registration errors are treated as `0.0`.
    pub below_registration_resolution: bool,
}

#[derive(Clone, Debug, PartialEq)]
/// Spatial graph over the input cell order.
///
/// Edges are undirected and stored once as sorted `(source, target)` index
/// pairs, ordered deterministically by ascending source and then target.
pub struct SpatialGraph {
    /// Number of input cells represented by graph nodes.
    pub n_nodes: usize,
    /// Deterministically ordered undirected edges.
    pub edges: Vec<SpatialEdge>,
}

impl SpatialGraph {
    pub(crate) fn estimated_storage_bytes(&self) -> usize {
        self.edges
            .capacity()
            .saturating_mul(size_of::<SpatialEdge>())
    }
}

#[derive(Debug)]
pub(crate) struct SpatialGraphBuild {
    pub(crate) graph: SpatialGraph,
    /// Conservative peak for graph-owned map, output, and kNN query scratch.
    pub(crate) estimated_peak_storage_bytes: usize,
}

/// Build a deterministic spatial graph over registered fused-cell coordinates.
///
/// Radius edges use an inclusive `distance <= radius_um` threshold. kNN edges
/// are the union of per-node nearest-neighbor choices, stored once as sorted
/// undirected index pairs. The registration-resolution flag uses strict
/// `distance < 2 * max(endpoint registration_error_um)` semantics.
#[cfg(test)]
pub fn build_spatial_graph(cells: &[FusedCell], config: GraphConfig) -> Result<SpatialGraph> {
    validate_config(config)?;
    validate_cells(cells)?;

    let index = SpatialIndex2D::from_points(
        cells
            .iter()
            .map(|cell| [cell.x_um_registered, cell.y_um_registered]),
    )?;
    Ok(build_validated_spatial_graph(cells, &index, config, usize::MAX)?.graph)
}

pub(crate) fn build_spatial_graph_with_index(
    cells: &[FusedCell],
    index: &SpatialIndex2D,
    config: GraphConfig,
    storage_budget_bytes: usize,
) -> Result<SpatialGraphBuild> {
    validate_config(config)?;
    validate_cells(cells)?;
    validate_index_length(cells, index)?;
    build_validated_spatial_graph(cells, index, config, storage_budget_bytes)
}

fn build_validated_spatial_graph(
    cells: &[FusedCell],
    index: &SpatialIndex2D,
    config: GraphConfig,
    storage_budget_bytes: usize,
) -> Result<SpatialGraphBuild> {
    #[cfg(test)]
    GRAPH_BUILD_CALLS.set(GRAPH_BUILD_CALLS.get() + 1);
    let scratch_bytes = config
        .k_nearest
        .map_or(0, |_| index.len().saturating_mul(size_of::<Neighbor>()));
    enforce_storage_budget("graph construction", scratch_bytes, storage_budget_bytes)?;
    let mut pairs = BTreeMap::new();
    if let Some(radius_um) = config.radius_um {
        add_radius_edges(
            index,
            radius_um,
            &mut pairs,
            scratch_bytes,
            storage_budget_bytes,
        )?;
    }
    if let Some(k_nearest) = config.k_nearest {
        add_knn_edges(
            index,
            k_nearest,
            &mut pairs,
            scratch_bytes,
            storage_budget_bytes,
        )?;
    }

    let estimated_peak_storage_bytes = graph_peak_storage_bytes(pairs.len(), scratch_bytes);

    let edges = pairs
        .into_iter()
        .map(|((source, target), distance_um)| build_edge(cells, source, target, distance_um))
        .collect();

    Ok(SpatialGraphBuild {
        graph: SpatialGraph {
            n_nodes: cells.len(),
            edges,
        },
        estimated_peak_storage_bytes,
    })
}

fn validate_index_length(cells: &[FusedCell], index: &SpatialIndex2D) -> Result<()> {
    if index.len() != cells.len() {
        return Err(MarklabError::Geometry(format!(
            "spatial index has {} points for {} graph cells",
            index.len(),
            cells.len()
        )));
    }
    Ok(())
}

fn validate_config(config: GraphConfig) -> Result<()> {
    if config.radius_um.is_none() && config.k_nearest.is_none() {
        return Err(MarklabError::Config(
            "neighborhood graph requires radius_um or k_nearest".into(),
        ));
    }

    if let Some(radius_um) = config.radius_um {
        if !radius_um.is_finite() || radius_um <= 0.0 {
            return Err(MarklabError::Config(
                "radius_um must be finite and positive".into(),
            ));
        }
    }

    if matches!(config.k_nearest, Some(0)) {
        return Err(MarklabError::Config(
            "k_nearest must be positive when configured".into(),
        ));
    }

    Ok(())
}

fn validate_cells(cells: &[FusedCell]) -> Result<()> {
    for (index, cell) in cells.iter().enumerate() {
        if !cell.x_um_registered.is_finite() || !cell.y_um_registered.is_finite() {
            return Err(MarklabError::Validation(format!(
                "cell {index} registered coordinates must be finite"
            )));
        }
        if let Some(error_um) = cell.registration_error_um {
            if !error_um.is_finite() || error_um < 0.0 {
                return Err(MarklabError::Validation(format!(
                    "cell {index} registration_error_um must be finite and non-negative"
                )));
            }
        }
    }
    Ok(())
}

fn add_radius_edges(
    index: &SpatialIndex2D,
    radius_um: f64,
    pairs: &mut BTreeMap<(usize, usize), f64>,
    scratch_bytes: usize,
    storage_budget_bytes: usize,
) -> Result<()> {
    for source in 0..index.len() {
        let mut insertion_error = None;
        index.visit_within_radius(source, radius_um, |neighbor| {
            if neighbor.index > source && insertion_error.is_none() {
                if let Err(error) = insert_pair_with_budget(
                    pairs,
                    (source, neighbor.index),
                    neighbor.distance_um,
                    scratch_bytes,
                    storage_budget_bytes,
                ) {
                    insertion_error = Some(error);
                }
            }
        })?;
        if let Some(error) = insertion_error {
            return Err(error);
        }
    }
    Ok(())
}

fn add_knn_edges(
    index: &SpatialIndex2D,
    k_nearest: usize,
    pairs: &mut BTreeMap<(usize, usize), f64>,
    scratch_bytes: usize,
    storage_budget_bytes: usize,
) -> Result<()> {
    for source in 0..index.len() {
        for neighbor in index.k_nearest(source, k_nearest)? {
            insert_pair_with_budget(
                pairs,
                normalized_pair(source, neighbor.index),
                neighbor.distance_um,
                scratch_bytes,
                storage_budget_bytes,
            )?;
        }
    }
    Ok(())
}

fn insert_pair_with_budget(
    pairs: &mut BTreeMap<(usize, usize), f64>,
    pair: (usize, usize),
    distance_um: f64,
    scratch_bytes: usize,
    storage_budget_bytes: usize,
) -> Result<()> {
    if pairs.contains_key(&pair) {
        return Ok(());
    }
    let next_count = pairs.len().saturating_add(1);
    enforce_storage_budget(
        "graph construction",
        graph_peak_storage_bytes(next_count, scratch_bytes),
        storage_budget_bytes,
    )?;
    pairs.insert(pair, distance_um);
    Ok(())
}

fn graph_peak_storage_bytes(edge_count: usize, scratch_bytes: usize) -> usize {
    // A BTreeMap node owns key/value storage plus child links and allocator
    // bookkeeping. Four machine words per entry is a conservative portable
    // allowance without depending on standard-library node internals.
    let map_entry_bytes = size_of::<(usize, usize)>()
        .saturating_add(size_of::<f64>())
        .saturating_add(4usize.saturating_mul(size_of::<usize>()));
    let per_edge_peak_bytes = map_entry_bytes.saturating_add(size_of::<SpatialEdge>());
    scratch_bytes.saturating_add(edge_count.saturating_mul(per_edge_peak_bytes))
}

fn build_edge(cells: &[FusedCell], source: usize, target: usize, distance_um: f64) -> SpatialEdge {
    let dx = cells[target].x_um_registered - cells[source].x_um_registered;
    let dy = cells[target].y_um_registered - cells[source].y_um_registered;
    let max_registration_error_um = cells[source]
        .registration_error_um
        .unwrap_or(0.0)
        .max(cells[target].registration_error_um.unwrap_or(0.0));

    SpatialEdge {
        source,
        target,
        distance_um,
        angle_rad: dy.atan2(dx),
        below_registration_resolution: distance_um < 2.0 * max_registration_error_um,
    }
}

fn normalized_pair(left: usize, right: usize) -> (usize, usize) {
    if left < right {
        (left, right)
    } else {
        (right, left)
    }
}
