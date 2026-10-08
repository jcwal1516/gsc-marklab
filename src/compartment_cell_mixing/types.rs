use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Fixed work and memory ceilings for one compartment cell-mixing graph.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompartmentCellMixingLimits {
    pub maximum_points: usize,
    pub maximum_distance_queries: usize,
    pub maximum_pair_visits: usize,
    pub maximum_retained_bytes: usize,
}

impl CompartmentCellMixingLimits {
    pub fn new(
        maximum_points: usize,
        maximum_distance_queries: usize,
        maximum_pair_visits: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, CompartmentCellMixingError> {
        if [
            maximum_points,
            maximum_distance_queries,
            maximum_pair_visits,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(CompartmentCellMixingError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_distance_queries,
            maximum_pair_visits,
            maximum_retained_bytes,
        })
    }
}

/// One fixed physical adjacency scale and its resource ceilings.
#[derive(Clone, Debug, PartialEq)]
pub struct CompartmentCellMixingConfig {
    pub(super) radius_um: f64,
    pub(super) limits: CompartmentCellMixingLimits,
}

impl CompartmentCellMixingConfig {
    pub fn new(
        radius_um: f64,
        limits: CompartmentCellMixingLimits,
    ) -> Result<Self, CompartmentCellMixingError> {
        if !radius_um.is_finite() || radius_um <= 0.0 || !(radius_um * radius_um).is_finite() {
            return Err(CompartmentCellMixingError::InvalidConfig);
        }
        Ok(Self { radius_um, limits })
    }

    pub fn radius_um(&self) -> f64 {
        self.radius_um
    }

    pub fn limits(&self) -> CompartmentCellMixingLimits {
        self.limits
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompartmentCellMixingSummary {
    pub compartment_id: String,
    pub cell_count: usize,
    pub same_compartment_neighbor_incidences: usize,
    pub cross_compartment_neighbor_incidences: usize,
    pub neighbor_label_entropy_nats: f64,
    pub normalized_neighbor_label_entropy: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompartmentCellMixingResult {
    pub case_id: String,
    pub timepoint: String,
    pub mark_id: String,
    pub measurement_status: String,
    pub coordinate_frame_id: String,
    pub partition_digest: String,
    pub radius_um: f64,
    pub graph_digest: String,
    pub configuration_digest: String,
    pub point_count: usize,
    pub undirected_edge_count: usize,
    pub directed_pair_visits: usize,
    pub cross_compartment_edge_count: usize,
    pub cross_compartment_edge_fraction: f64,
    pub random_label_cross_edge_expectation: f64,
    pub cross_edge_fraction_minus_expectation: f64,
    pub edge_type_entropy_nats: f64,
    pub normalized_edge_type_entropy: f64,
    pub negative: CompartmentCellMixingSummary,
    pub positive: CompartmentCellMixingSummary,
    pub estimated_storage_bytes: usize,
    pub limits: CompartmentCellMixingLimits,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum CompartmentCellMixingError {
    #[error("compartment cell-mixing resource limits must be positive")]
    InvalidResourceLimit,
    #[error("compartment cell-mixing radius must be finite, positive, and square to finite")]
    InvalidConfig,
    #[error("compartment cell-mixing interface profile failed: {reason}")]
    InterfaceProfile { reason: String },
    #[error("compartment cell-mixing geometry failed: {reason}")]
    Geometry { reason: String },
    #[error("compartment cell-mixing graph has no adjacency edges")]
    NoAdjacencyEdges,
    #[error("compartment cell-mixing pair visits exceeded {maximum} at {observed}")]
    PairVisitLimitExceeded { observed: usize, maximum: usize },
    #[error("compartment cell-mixing requires {required} bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded { required: usize, maximum: usize },
    #[error("compartment cell-mixing size arithmetic overflow")]
    SizeOverflow,
}
