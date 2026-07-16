use std::{collections::BTreeMap, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    errors::{MmrspaceError, Result},
    geom::mask::TumorMask,
    io::load_pattern_path,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Pattern {
    pub x_um: Box<[f64]>,
    pub y_um: Box<[f64]>,
    pub mark: Box<[u8]>,
    pub mark_prob: Option<Box<[f32]>>,
    pub tumor_probability: Option<Box<[f32]>>,
    pub nucleus_area_um2: Option<Box<[f32]>>,
    pub valid: Box<[u8]>,
    pub component_id: Option<Box<[u32]>>,
    pub qc_bin: Option<Box<[u16]>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub categorical_strata: BTreeMap<String, Box<[u32]>>,
    pub local_dab_od: Option<Box<[f32]>>,
    pub local_hematoxylin_od: Option<Box<[f32]>>,
    pub internal_control_valid_fraction: Option<f64>,
    pub artifact_excluded_fraction: Option<f64>,
    pub nonviable_excluded_fraction: Option<f64>,
    pub window: TumorWindow,
    pub meta: PatternMeta,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PatternMeta {
    pub case_id: String,
    pub timepoint: String,
    pub protein: String,
    pub slide_id: Option<String>,
    pub section_id: Option<String>,
    pub stain_batch: Option<String>,
    pub block_id: Option<String>,
    pub region_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct TumorWindow {
    pub area_um2: f64,
    pub l_eff_um: f64,
    pub d_nn_mean_um: f64,
    pub valid_mask_fraction: f64,
}

impl Default for TumorWindow {
    fn default() -> Self {
        Self {
            area_um2: 0.0,
            l_eff_um: 0.0,
            d_nn_mean_um: 0.0,
            valid_mask_fraction: 1.0,
        }
    }
}

impl Pattern {
    pub fn from_paths(cells: impl AsRef<Path>, mask: impl AsRef<Path>) -> Result<Self> {
        let mask_path = mask.as_ref();
        let mask_text = std::fs::read_to_string(mask_path)
            .map_err(|source| MmrspaceError::io(mask_path, source))?;
        let mask = TumorMask::from_geojson_str(&mask_text)?;
        load_pattern_path(cells, &mask)
    }

    pub fn from_arrays(
        x_um: Vec<f64>,
        y_um: Vec<f64>,
        mark: Vec<u8>,
        meta: PatternMeta,
    ) -> Result<Self> {
        if x_um.len() != y_um.len() || x_um.len() != mark.len() {
            return Err(MmrspaceError::Schema(
                "x_um, y_um, and mark arrays must have equal length".into(),
            ));
        }

        if mark.iter().any(|value| *value != 0 && *value != 1) {
            return Err(MmrspaceError::Schema(
                "mark must be binary with values 0 or 1".into(),
            ));
        }

        if x_um
            .iter()
            .chain(y_um.iter())
            .any(|value| !value.is_finite())
        {
            return Err(MmrspaceError::Schema(
                "coordinates must be finite f64 values".into(),
            ));
        }

        let valid = vec![1; mark.len()].into_boxed_slice();

        Ok(Self {
            x_um: x_um.into_boxed_slice(),
            y_um: y_um.into_boxed_slice(),
            mark: mark.into_boxed_slice(),
            mark_prob: None,
            tumor_probability: None,
            nucleus_area_um2: None,
            valid,
            component_id: None,
            qc_bin: None,
            categorical_strata: BTreeMap::new(),
            local_dab_od: None,
            local_hematoxylin_od: None,
            internal_control_valid_fraction: None,
            artifact_excluded_fraction: None,
            nonviable_excluded_fraction: None,
            window: TumorWindow::default(),
            meta,
        })
    }

    pub fn len(&self) -> usize {
        self.mark.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mark.is_empty()
    }

    pub fn n_marked(&self) -> usize {
        self.mark.iter().filter(|value| **value == 1).count()
    }

    pub fn n_unmarked(&self) -> usize {
        self.len().saturating_sub(self.n_marked())
    }

    pub fn p_hat(&self) -> f64 {
        if self.is_empty() {
            0.0
        } else {
            self.n_marked() as f64 / self.len() as f64
        }
    }
}
