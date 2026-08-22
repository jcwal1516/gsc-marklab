use geojson::{GeoJson, Geometry, Value};

use crate::errors::{MarklabError, Result};

#[derive(Clone, Debug, PartialEq)]
pub struct TumorMask {
    polygons: Vec<MaskPolygon>,
    area_um2: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct MaskPolygon {
    exterior: Ring,
    holes: Vec<Ring>,
}

type Ring = Vec<(f64, f64)>;

impl TumorMask {
    pub fn from_geojson_str(text: &str) -> Result<Self> {
        let geojson = text
            .parse::<GeoJson>()
            .map_err(|err| MarklabError::Geometry(err.to_string()))?;
        let geometry = match geojson {
            GeoJson::Geometry(geometry) => geometry,
            GeoJson::Feature(feature) => feature.geometry.ok_or_else(|| {
                MarklabError::Geometry("GeoJSON feature must contain MultiPolygon geometry".into())
            })?,
            GeoJson::FeatureCollection(collection) if collection.features.len() == 1 => collection
                .features
                .into_iter()
                .next()
                .and_then(|feature| feature.geometry)
                .ok_or_else(|| {
                    MarklabError::Geometry(
                        "GeoJSON feature collection must contain MultiPolygon geometry".into(),
                    )
                })?,
            GeoJson::FeatureCollection(_) => {
                return Err(MarklabError::Geometry(
                    "mask input must be a single GeoJSON MultiPolygon".into(),
                ));
            }
        };

        Self::from_geometry(geometry)
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.polygons.iter().any(|polygon| polygon.contains(x, y))
    }

    pub fn area_um2(&self) -> f64 {
        self.area_um2
    }

    pub fn equivalent_area_diameter_um(&self) -> f64 {
        super::length_scales::equivalent_area_diameter_um(self.area_um2).unwrap_or(0.0)
    }

    fn from_geometry(geometry: Geometry) -> Result<Self> {
        let Value::MultiPolygon(raw_polygons) = geometry.value else {
            return Err(MarklabError::Geometry(
                "mask input must be a GeoJSON MultiPolygon".into(),
            ));
        };

        let mut polygons = Vec::with_capacity(raw_polygons.len());
        let mut total_area = 0.0;

        for raw_polygon in raw_polygons {
            if raw_polygon.is_empty() {
                return Err(MarklabError::Geometry(
                    "MultiPolygon polygon must contain an exterior ring".into(),
                ));
            }

            let exterior = parse_ring(&raw_polygon[0])?;
            let holes = raw_polygon[1..]
                .iter()
                .map(|ring| parse_ring(ring))
                .collect::<Result<Vec<_>>>()?;

            let area = ring_area(&exterior) - holes.iter().map(ring_area).sum::<f64>();
            total_area += area.max(0.0);
            polygons.push(MaskPolygon { exterior, holes });
        }

        Ok(Self {
            polygons,
            area_um2: total_area,
        })
    }
}

impl MaskPolygon {
    fn contains(&self, x: f64, y: f64) -> bool {
        point_in_ring(x, y, &self.exterior)
            && !self.holes.iter().any(|hole| point_in_ring(x, y, hole))
    }
}

fn parse_ring(raw: &[Vec<f64>]) -> Result<Ring> {
    if raw.len() < 4 {
        return Err(MarklabError::Geometry(
            "GeoJSON linear ring must contain at least four positions".into(),
        ));
    }

    let mut ring = Vec::with_capacity(raw.len());
    for position in raw {
        if position.len() < 2 || !position[0].is_finite() || !position[1].is_finite() {
            return Err(MarklabError::Geometry(
                "GeoJSON ring positions must contain finite x/y coordinates".into(),
            ));
        }
        ring.push((position[0], position[1]));
    }

    if ring.first() != ring.last() {
        return Err(MarklabError::Geometry(
            "GeoJSON linear ring must be closed".into(),
        ));
    }

    Ok(ring)
}

fn ring_area(ring: &Ring) -> f64 {
    ring.windows(2)
        .map(|pair| pair[0].0 * pair[1].1 - pair[1].0 * pair[0].1)
        .sum::<f64>()
        .abs()
        * 0.5
}

fn point_in_ring(x: f64, y: f64, ring: &Ring) -> bool {
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (xi, yi) = ring[i];
        let (xj, yj) = ring[j];
        let intersects =
            ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / ((yj - yi) + f64::EPSILON) + xi);
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
}
