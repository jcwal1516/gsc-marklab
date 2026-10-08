use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Fixed resource ceilings for one descriptive per-cell interface profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompartmentInterfaceLimits {
    /// Maximum row-aligned cells.
    pub maximum_points: usize,
    /// Maximum signed-interface distance queries.
    pub maximum_distance_queries: usize,
    /// Maximum retained result and working bytes.
    pub maximum_retained_bytes: usize,
}

impl CompartmentInterfaceLimits {
    /// Validate positive explicit point, query, and memory ceilings.
    pub fn new(
        maximum_points: usize,
        maximum_distance_queries: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, CompartmentInterfaceError> {
        if [
            maximum_points,
            maximum_distance_queries,
            maximum_retained_bytes,
        ]
        .contains(&0)
        {
            return Err(CompartmentInterfaceError::InvalidResourceLimit);
        }
        Ok(Self {
            maximum_points,
            maximum_distance_queries,
            maximum_retained_bytes,
        })
    }
}

/// One stable cell's descriptive signed distance to the shared compartment interface.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompartmentInterfaceCell {
    /// Exact row in the aligned Pattern and MarkTable.
    pub row: usize,
    /// Stable cell identity.
    pub cell_id: String,
    /// Declared categorical compartment identity.
    pub compartment_id: String,
    /// Positive-side/negative-side oriented interface distance in micrometres.
    pub signed_interface_distance_um: f64,
}

/// Per-compartment descriptive summary without cell-level replication claims.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompartmentInterfaceSummary {
    /// Exact compartment identity.
    pub compartment_id: String,
    /// Number of aligned cells in this compartment.
    pub cell_count: usize,
    /// Minimum signed distance among its cells.
    pub minimum_signed_distance_um: f64,
    /// Maximum signed distance among its cells.
    pub maximum_signed_distance_um: f64,
    /// Arithmetic mean absolute interface distance among its cells.
    pub mean_absolute_distance_um: f64,
}

/// Typed per-specimen cell-to-compartment-interface profile.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompartmentInterfaceProfile {
    /// Pattern case identity.
    pub case_id: String,
    /// Pattern timepoint identity.
    pub timepoint: String,
    /// Exact categorical mark identity.
    pub mark_id: String,
    /// Column-wide measurement status.
    pub measurement_status: String,
    /// Compartment assigned negative distance.
    pub negative_compartment_id: String,
    /// Compartment assigned positive distance.
    pub positive_compartment_id: String,
    /// Exact bound coordinate frame.
    pub coordinate_frame_id: String,
    /// Shared-interface length in micrometres.
    pub interface_length_um: f64,
    /// Exact partition identity.
    pub partition_digest: String,
    /// Exact table, partition, and limits identity.
    pub configuration_digest: String,
    /// Number of executed distance queries.
    pub query_count: usize,
    /// Conservative retained result/working byte estimate.
    pub estimated_storage_bytes: usize,
    /// Exact resource ceilings.
    pub limits: CompartmentInterfaceLimits,
    /// Stable row-aligned cell results.
    pub rows: Vec<CompartmentInterfaceCell>,
    /// Negative then positive compartment summaries.
    pub summaries: [CompartmentInterfaceSummary; 2],
}

/// Failure to construct a typed per-cell interface profile.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum CompartmentInterfaceError {
    /// One or more caller ceilings are zero.
    #[error("compartment-interface resource limits must be positive")]
    InvalidResourceLimit,
    /// The declared input does not contain the typed compartment column.
    #[error("compartment-interface profile requires a typed histologic_compartment column")]
    MissingCompartmentMark,
    /// A partition compartment ID is absent or duplicated in the categorical codebook.
    #[error("partition compartment {compartment_id} is not uniquely represented in the codebook")]
    UnresolvedCompartmentLevel {
        /// Missing or duplicated compartment identity.
        compartment_id: String,
    },
    /// A categorical row contains a third level unsupported by the binary partition.
    #[error("compartment row {row} has unsupported code {code}")]
    UnsupportedCompartmentCode {
        /// Pattern/MarkTable row.
        row: usize,
        /// Observed categorical code.
        code: u32,
    },
    /// The exact coordinate frames differ.
    #[error("compartment-interface input and partition coordinate frames differ")]
    CoordinateFrameMismatch,
    /// Point count exceeds the caller ceiling.
    #[error("compartment-interface input has {observed} points; maximum is {maximum}")]
    PointLimitExceeded {
        /// Observed rows.
        observed: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Required distance queries exceed the caller ceiling.
    #[error("compartment-interface profile requires {required} queries; maximum is {maximum}")]
    DistanceQueryLimitExceeded {
        /// Required point queries.
        required: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// Retained storage exceeds the caller ceiling.
    #[error("compartment-interface profile requires {required} bytes; maximum is {maximum}")]
    RetainedByteLimitExceeded {
        /// Conservative required bytes.
        required: usize,
        /// Caller ceiling.
        maximum: usize,
    },
    /// A typed row label contradicts its non-interface polygon membership.
    #[error("compartment row {row} label {label} contradicts signed distance {distance_um}")]
    SpatialLabelMismatch {
        /// Pattern/MarkTable row.
        row: usize,
        /// Declared compartment label.
        label: String,
        /// Geometry-derived signed distance.
        distance_um: f64,
    },
    /// A point could not be queried against the partition.
    #[error("compartment-interface row {row} geometry query failed: {reason}")]
    GeometryQuery {
        /// Pattern/MarkTable row.
        row: usize,
        /// Deterministic query failure.
        reason: String,
    },
    /// The declared input identity changed or is invalid.
    #[error("compartment-interface declared input is invalid: {reason}")]
    InvalidDeclaredInput {
        /// Deterministic input failure.
        reason: String,
    },
    /// A required compartment has no rows.
    #[error("compartment {compartment_id} has no cells")]
    EmptyCompartment {
        /// Empty compartment identity.
        compartment_id: String,
    },
    /// Checked size arithmetic overflowed.
    #[error("compartment-interface size arithmetic overflow")]
    SizeOverflow,
    /// Allocation failed under the validated ceiling.
    #[error("compartment-interface allocation failed")]
    AllocationFailed,
}
