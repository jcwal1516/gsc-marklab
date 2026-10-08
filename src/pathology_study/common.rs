use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace, CoordinateUnit,
    MarklabError, ObservationWindow2D, ObservationWindowLimits, Result, SpatialAxis,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe<D> {
    pub format: String,
    pub version: u32,
    pub study: Study,
    pub design: D,
    pub limits: Limits,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Study {
    pub study_id: String,
    pub phenotypes: Phenotypes,
    pub slides: Vec<Slide>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Phenotypes {
    pub names: Vec<String>,
    pub measurement_status: String,
    pub provenance: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Slide {
    pub slide_id: String,
    pub patient_id: String,
    pub group: String,
    pub coordinate_frame_id: String,
    pub window: serde_json::Value,
    pub cells: Vec<Cell>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    pub id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub phenotype: String,
    pub stratum: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub maximum_cells: usize,
    pub maximum_work: usize,
    pub maximum_geometry_vertices: usize,
    pub maximum_tiles: usize,
    pub memory_budget_bytes: usize,
}

pub struct Budget {
    pub limits: Limits,
    pub work: usize,
    pub tiles: usize,
    pub bytes: usize,
}

impl Budget {
    pub fn work(&mut self, count: usize) -> Result<()> {
        self.work = self
            .work
            .checked_add(count)
            .ok_or_else(|| invalid("work overflow"))?;
        if self.work > self.limits.maximum_work {
            return Err(invalid("work resource limit exceeded"));
        }
        Ok(())
    }
    pub fn retain(&mut self, bytes: usize) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| invalid("memory overflow"))?;
        if self.bytes > self.limits.memory_budget_bytes {
            return Err(invalid("memory resource limit exceeded"));
        }
        Ok(())
    }
    pub fn tiles(&mut self, count: usize) -> Result<()> {
        self.tiles = self
            .tiles
            .checked_add(count)
            .ok_or_else(|| invalid("tile overflow"))?;
        if self.tiles > self.limits.maximum_tiles {
            return Err(invalid("tile resource limit exceeded"));
        }
        self.retain(
            count
                .checked_mul(512)
                .ok_or_else(|| invalid("tile storage overflow"))?,
        )
    }
    pub fn geometry(&mut self, window: &ObservationWindow2D, vertices: usize) -> Result<()> {
        let n = window
            .translation_segment_count()
            .checked_add(vertices)
            .ok_or_else(|| invalid("geometry overflow"))?;
        self.work(
            n.checked_mul(n)
                .ok_or_else(|| invalid("geometry work overflow"))?,
        )
    }
}

pub fn invalid(message: impl Into<String>) -> MarklabError {
    MarklabError::Validation(format!("pathology study: {}", message.into()))
}

pub fn text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

pub fn finite(value: f64) -> Result<f64> {
    if !value.is_finite() {
        Err(invalid("nonfinite numerical result"))
    } else {
        Ok(if value == 0.0 { 0.0 } else { value })
    }
}

pub fn parse<D: serde::de::DeserializeOwned>(
    bytes: &[u8],
    format: &str,
) -> Result<(Recipe<D>, Vec<ObservationWindow2D>, Budget)> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(invalid("recipe exceeds 64 MiB"));
    }
    let mut recipe: Recipe<D> = serde_json::from_slice(bytes)?;
    let l = recipe.limits;
    if recipe.format != format
        || recipe.version != 1
        || !text(&recipe.study.study_id)
        || l.maximum_cells == 0
        || l.maximum_cells > 1_000_000
        || l.maximum_work == 0
        || l.maximum_work as u64 > 50_000_000_000
        || l.maximum_geometry_vertices < 4
        || l.maximum_geometry_vertices > 1_000_000
        || l.maximum_tiles == 0
        || l.maximum_tiles > 1_000_000
        || l.memory_budget_bytes == 0
        || l.memory_budget_bytes as u64 > 8 * 1024 * 1024 * 1024
    {
        return Err(invalid("invalid recipe identity or resource limits"));
    }
    let mut budget = Budget {
        limits: l,
        work: 0,
        tiles: 0,
        bytes: 0,
    };
    budget.retain(
        bytes
            .len()
            .checked_mul(6)
            .ok_or_else(|| invalid("recipe memory overflow"))?,
    )?;
    let phenotypes = &recipe.study.phenotypes;
    if phenotypes.names.is_empty()
        || phenotypes.names.len() > 64
        || phenotypes.names.iter().any(|s| !text(s))
        || phenotypes.names.iter().collect::<BTreeSet<_>>().len() != phenotypes.names.len()
        || !text(&phenotypes.provenance)
        || crate::measurement_status_wire::parse(&phenotypes.measurement_status).is_none()
    {
        return Err(invalid("invalid phenotype codebook or provenance"));
    }
    if recipe.study.slides.is_empty() || recipe.study.slides.len() > 512 {
        return Err(invalid("invalid slide count"));
    }
    recipe
        .study
        .slides
        .sort_by(|a, b| a.slide_id.cmp(&b.slide_id));
    let mut slide_ids = BTreeSet::new();
    let mut patient_groups = BTreeMap::new();
    let mut windows = Vec::new();
    let mut cells = 0usize;
    for slide in &mut recipe.study.slides {
        if !text(&slide.slide_id)
            || !text(&slide.patient_id)
            || !text(&slide.group)
            || !slide_ids.insert(slide.slide_id.clone())
        {
            return Err(invalid("invalid/duplicate study identity"));
        }
        if let Some(group) = patient_groups.insert(slide.patient_id.clone(), slide.group.clone()) {
            if group != slide.group {
                return Err(invalid("patient assigned to multiple groups"));
            }
        }
        cells = cells
            .checked_add(slide.cells.len())
            .ok_or_else(|| invalid("cell count overflow"))?;
        if cells > l.maximum_cells {
            return Err(invalid("cell resource limit exceeded"));
        }
        budget.retain(slide.cells.len().saturating_mul(512))?;
        let window = framed_window(&slide.window, &slide.coordinate_frame_id, l)?;
        budget.retain(window.boundary_storage_bytes())?;
        budget.work(
            window
                .translation_segment_count()
                .saturating_mul(window.translation_segment_count()),
        )?;
        slide.cells.sort_by(|a, b| a.id.cmp(&b.id));
        let mut ids = BTreeSet::new();
        let mut positions = BTreeSet::new();
        for cell in &slide.cells {
            let bits = |v: f64| if v == 0.0 { 0 } else { v.to_bits() };
            if !text(&cell.id)
                || !text(&cell.stratum)
                || !ids.insert(&cell.id)
                || !cell.x_um.is_finite()
                || !cell.y_um.is_finite()
                || !positions.insert((bits(cell.x_um), bits(cell.y_um)))
                || !window.contains(cell.x_um, cell.y_um)
                || !phenotypes.names.contains(&cell.phenotype)
            {
                return Err(invalid(
                    "invalid cell identity, coordinates, hard phenotype or stratum",
                ));
            }
        }
        windows.push(window);
    }
    Ok((recipe, windows, budget))
}

pub fn inference_design(a: &str, b: &str, permutations: usize, alpha: f64) -> Result<()> {
    if !text(a)
        || !text(b)
        || a == b
        || !(0.0 < alpha && alpha < 0.5)
        || !(1..=100_000).contains(&permutations)
        || (permutations + 1) as f64 * alpha < 1.0
    {
        return Err(invalid(
            "invalid groups, alpha or finite permutation resolution",
        ));
    }
    Ok(())
}

pub fn polygon_json(geometry: &geo::MultiPolygon<f64>) -> serde_json::Value {
    let coordinates = geometry
        .0
        .iter()
        .map(|p| {
            std::iter::once(p.exterior())
                .chain(p.interiors())
                .map(|r| r.0.iter().map(|c| [c.x, c.y]).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    serde_json::json!({"type":"MultiPolygon","coordinates":coordinates})
}

pub fn framed_window(
    value: &serde_json::Value,
    frame_id: &str,
    l: Limits,
) -> Result<ObservationWindow2D> {
    let frame = CoordinateFrameId::new(frame_id).map_err(|e| invalid(e.to_string()))?;
    let registry = CoordinateRegistry::new(
        vec![CoordinateFrame::new(
            frame.clone(),
            vec![SpatialAxis::X, SpatialAxis::Y],
            CoordinateUnit::Micrometer,
            CoordinateSpace::Physical,
        )
        .map_err(|e| invalid(e.to_string()))?],
        vec![],
        vec![],
        vec![],
    )
    .map_err(|e| invalid(e.to_string()))?;
    let window = ObservationWindow2D::from_geojson_str(
        &serde_json::to_string(value)?,
        ObservationWindowLimits::new(
            64 << 20,
            1024,
            4096,
            l.maximum_geometry_vertices,
            l.maximum_work,
        )
        .map_err(|e| invalid(e.to_string()))?,
    )
    .and_then(|w| w.with_coordinate_frame(&registry, frame))
    .map_err(|e| invalid(e.to_string()))?;
    Ok(window)
}
