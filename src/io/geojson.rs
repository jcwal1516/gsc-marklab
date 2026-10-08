use std::path::Path;

use crate::{
    errors::{MarklabError, Result},
    output::{NeighborhoodTerritory, ResidualTerritory},
};

pub(crate) fn write_residual_territories(
    territories: &[ResidualTerritory],
    path: impl AsRef<Path>,
) -> Result<()> {
    let features = territories
        .iter()
        .map(|territory| -> Result<_> {
            let ring = territory_polygon_ring(
                territory.center_x_um,
                territory.center_y_um,
                territory.radius_um,
                32,
            )?;
            Ok(serde_json::json!({
                "type": "Feature",
                "geometry": {
                    "type": "Polygon",
                    "coordinates": [ring]
                },
                "properties": {
                    "center_x_um": territory.center_x_um,
                    "center_y_um": territory.center_y_um,
                    "radius_um": territory.radius_um,
                    "analysis_scale_um": territory.analysis_scale_um,
                    "residual_score": territory.residual_score,
                    "supporting_marked_cells": territory.supporting_marked_cells,
                    "component_id": territory.component_id
                }
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    write_feature_collection(features, path)
}

pub(crate) fn write_neighborhood_territories(
    territories: &[NeighborhoodTerritory],
    path: impl AsRef<Path>,
) -> Result<()> {
    let features = territories
        .iter()
        .map(|territory| -> Result<_> {
            let ring = territory_polygon_ring(
                territory.center_x_um,
                territory.center_y_um,
                territory.radius_um,
                32,
            )?;
            Ok(serde_json::json!({
                "type": "Feature",
                "geometry": {
                    "type": "Polygon",
                    "coordinates": [ring]
                },
                "properties": {
                    "center_x_um": territory.center_x_um,
                    "center_y_um": territory.center_y_um,
                    "radius_um": territory.radius_um,
                    "supporting_abnormal_cells": territory.supporting_abnormal_cells,
                    "cluster_id": territory.cluster_id
                }
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    write_feature_collection(features, path)
}

fn write_feature_collection(
    features: Vec<serde_json::Value>,
    path: impl AsRef<Path>,
) -> Result<()> {
    if features.is_empty() {
        return Ok(());
    }
    let geojson = serde_json::json!({
        "type": "FeatureCollection",
        "features": features
    });
    let path = path.as_ref();
    std::fs::write(path, geojson.to_string()).map_err(|source| MarklabError::io(path, source))
}

fn territory_polygon_ring(
    center_x_um: f64,
    center_y_um: f64,
    radius_um: f64,
    segments: usize,
) -> Result<Vec<[f64; 2]>> {
    if !center_x_um.is_finite() || !center_y_um.is_finite() {
        return Err(MarklabError::Compute(
            "territory center coordinates must be finite".into(),
        ));
    }
    if !radius_um.is_finite() || radius_um <= 0.0 {
        return Err(MarklabError::Compute(
            "territory radius must be finite and positive".into(),
        ));
    }
    let n = segments.max(8);
    let mut ring = Vec::with_capacity(n + 1);
    for index in 0..n {
        let theta = 2.0 * std::f64::consts::PI * index as f64 / n as f64;
        ring.push([
            center_x_um + radius_um * theta.cos(),
            center_y_um + radius_um * theta.sin(),
        ]);
    }
    if let Some(first) = ring.first().copied() {
        ring.push(first);
    }
    Ok(ring)
}
