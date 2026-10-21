//! Geometric records shared by analysis specifications and worker protocols.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize)]
/// Axis-aligned rectangular observation window in micrometres.
pub struct RectangularWindow {
    /// Minimum x coordinate, in micrometres.
    pub xmin_um: f64,
    /// Minimum y coordinate, in micrometres.
    pub ymin_um: f64,
    /// Maximum x coordinate, in micrometres.
    pub xmax_um: f64,
    /// Maximum y coordinate, in micrometres.
    pub ymax_um: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
/// Covariate and offset attached to a joint-model location grid cell.
pub struct JointLocationGridRow {
    /// Zero-based x grid index.
    pub ix: u32,
    /// Zero-based y grid index.
    pub iy: u32,
    /// Location-model covariate value.
    pub location_covariate: f64,
    /// Location-model log-intensity offset.
    pub location_offset: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
/// Logical geometry and identity of an arbitrary observation window.
pub struct ArbitraryWindowIppWindowSummary {
    /// Observed area, in square micrometres.
    pub area_um2: f64,
    /// Total boundary perimeter, in micrometres.
    pub perimeter_um: f64,
    /// Bounding box as minimum x, minimum y, maximum x, maximum y.
    pub bounds_um: [f64; 4],
    /// Number of disconnected polygon components.
    pub component_count: usize,
    /// Number of interior polygon holes.
    pub hole_count: usize,
    /// Total number of exterior and interior rings.
    pub ring_count: usize,
    /// Total number of represented boundary vertices.
    pub vertex_count: usize,
    /// Digest of the canonical logical window representation.
    pub logical_digest: String,
}

/// One positive-area quadrature node in an exact observation window.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArbitraryWindowIppQuadratureNode {
    /// Stable identity of this quadrature node.
    pub node_id: String,
    /// Node x coordinate, in micrometres.
    pub x_um: f64,
    /// Node y coordinate, in micrometres.
    pub y_um: f64,
    /// Positive integration area represented by this node.
    pub weight_um2: f64,
    /// Fixed location-model covariate value.
    pub covariate: f64,
    /// Fixed location-model log-intensity offset.
    pub offset: f64,
}
