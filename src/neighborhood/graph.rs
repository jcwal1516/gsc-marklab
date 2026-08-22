use std::collections::BTreeMap;

use crate::{
    errors::{MarklabError, Result},
    geom::spatial_index::SpatialIndex2D,
    multimodal::cells::FusedCell,
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

/// Build a deterministic spatial graph over registered fused-cell coordinates.
///
/// Radius edges use an inclusive `distance <= radius_um` threshold. kNN edges
/// are the union of per-node nearest-neighbor choices, stored once as sorted
/// undirected index pairs. The registration-resolution flag uses strict
/// `distance < 2 * max(endpoint registration_error_um)` semantics.
pub fn build_spatial_graph(cells: &[FusedCell], config: GraphConfig) -> Result<SpatialGraph> {
    #[cfg(test)]
    GRAPH_BUILD_CALLS.set(GRAPH_BUILD_CALLS.get() + 1);
    validate_config(config)?;
    validate_cells(cells)?;

    let index = SpatialIndex2D::from_points(
        cells
            .iter()
            .map(|cell| [cell.x_um_registered, cell.y_um_registered]),
    )?;
    let mut pairs = BTreeMap::new();
    if let Some(radius_um) = config.radius_um {
        add_radius_edges(&index, radius_um, &mut pairs)?;
    }
    if let Some(k_nearest) = config.k_nearest {
        add_knn_edges(&index, k_nearest, &mut pairs)?;
    }

    let edges = pairs
        .into_iter()
        .map(|((source, target), distance_um)| build_edge(cells, source, target, distance_um))
        .collect();

    Ok(SpatialGraph {
        n_nodes: cells.len(),
        edges,
    })
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
) -> Result<()> {
    for source in 0..index.len() {
        for neighbor in index.within_radius(source, radius_um)? {
            if neighbor.index > source {
                pairs.insert((source, neighbor.index), neighbor.distance_um);
            }
        }
    }
    Ok(())
}

fn add_knn_edges(
    index: &SpatialIndex2D,
    k_nearest: usize,
    pairs: &mut BTreeMap<(usize, usize), f64>,
) -> Result<()> {
    for source in 0..index.len() {
        for neighbor in index.k_nearest(source, k_nearest)? {
            pairs
                .entry(normalized_pair(source, neighbor.index))
                .or_insert(neighbor.distance_um);
        }
    }
    Ok(())
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
