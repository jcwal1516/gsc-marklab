use std::io;

use marklab_workflow::{ArtifactRef, NodeError};
use serde::Serialize;

use crate::ObservationWindow2D;

const FRAME_BOUND_WINDOW_KIND: &str = "application/vnd.marklab.observation-window-ref;version=1";
const FULL_WINDOW_KIND: &str = "application/vnd.marklab.observation-window-2d+json;version=1";

#[derive(Serialize)]
struct FrameBoundWindowArtifact<'a> {
    logical_digest: String,
    coordinate_frame_id: &'a str,
}

pub(crate) fn frame_bound_window_artifact(
    window: &ObservationWindow2D,
) -> Result<ArtifactRef, NodeError> {
    let frame = window.coordinate_frame_id().ok_or_else(|| {
        NodeError::input(io::Error::new(
            io::ErrorKind::InvalidData,
            "observation window is not frame-bound",
        ))
    })?;
    let bytes = serde_json::to_vec(&FrameBoundWindowArtifact {
        logical_digest: window.descriptor().logical_digest.to_string(),
        coordinate_frame_id: frame.as_str(),
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(FRAME_BOUND_WINDOW_KIND, &bytes).map_err(NodeError::input)
}

#[derive(Serialize)]
struct FullWindowArtifact {
    kind: &'static str,
    logical_digest: String,
    area_um2: f64,
    perimeter_um: f64,
    bounds_um: [f64; 4],
    component_count: usize,
    hole_count: usize,
    ring_count: usize,
    vertex_count: usize,
}

pub(crate) fn full_window_artifact(window: &ObservationWindow2D) -> Result<ArtifactRef, NodeError> {
    let descriptor = window.descriptor();
    let bytes = serde_json::to_vec(&FullWindowArtifact {
        kind: "marklab.observation_window_2d",
        logical_digest: descriptor.logical_digest.to_string(),
        area_um2: descriptor.area_um2,
        perimeter_um: descriptor.perimeter_um,
        bounds_um: descriptor.bounds_um,
        component_count: descriptor.component_count,
        hole_count: descriptor.hole_count,
        ring_count: descriptor.ring_count,
        vertex_count: descriptor.vertex_count,
    })
    .map_err(NodeError::input)?;
    ArtifactRef::from_bytes(FULL_WINDOW_KIND, &bytes).map_err(NodeError::input)
}
