use crate::errors::{MarklabError, Result};
use crate::geom::window::{ObservationWindow2D, ObservationWindowLimits};

#[derive(Clone, Debug, PartialEq)]
pub struct TumorMask {
    window: ObservationWindow2D,
}

impl TumorMask {
    pub fn from_geojson_str(text: &str) -> Result<Self> {
        ObservationWindow2D::from_geojson_str(text, ObservationWindowLimits::default())
            .map(|window| Self { window })
            .map_err(|error| MarklabError::Geometry(error.to_string()))
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.window.contains(x, y)
    }

    pub fn area_um2(&self) -> f64 {
        self.window.area_um2()
    }

    pub fn equivalent_area_diameter_um(&self) -> f64 {
        super::length_scales::equivalent_area_diameter_um(self.window.area_um2()).unwrap_or(0.0)
    }

    #[allow(dead_code, reason = "used by the feature-gated classical CLI adapter")]
    pub(crate) fn from_observation_window(window: ObservationWindow2D) -> Self {
        Self { window }
    }
}
