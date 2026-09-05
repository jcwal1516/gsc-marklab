//! Paired measured-patient Gaussian probabilistic CCA fitted by bounded EM.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::BayesError;

mod matrix;
mod native;

pub use native::{fit_pcca_em, NativePccaBackend, PccaEmFit};

/// One admitted measured Gaussian modality.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PccaEmModality {
    pub id: String,
    pub measurement_status: String,
    pub likelihood: String,
    pub feature_names: Vec<String>,
}

/// Exact paired-patient design used by the two Gaussian views.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PccaEmDesign {
    pub entity_level: String,
    pub modality_x: PccaEmModality,
    pub modality_y: PccaEmModality,
    pub missingness_assumption: String,
    pub coordinate_frame: Option<String>,
}

/// A complete row from both views and its fixed train/test assignment.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PccaEmRow {
    pub entity_id: String,
    pub split: String,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

/// Controls and complete paired data for the EM specialization.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PccaEmSpec {
    pub design: PccaEmDesign,
    pub rows: Vec<PccaEmRow>,
    pub latent_dimensions: usize,
    pub regularization: f64,
    pub noise_floor: f64,
    pub maximum_iterations: usize,
    pub convergence_tolerance: f64,
    pub timeout_seconds: u64,
}

impl PccaEmSpec {
    pub(crate) fn validated(mut self) -> Result<Self, BayesError> {
        let dx = self.design.modality_x.feature_names.len();
        let dy = self.design.modality_y.feature_names.len();
        let valid_design = self.design.entity_level == "patient"
            && self.design.modality_x.id != self.design.modality_y.id
            && self.design.modality_x.measurement_status == "measured"
            && self.design.modality_y.measurement_status == "measured"
            && self.design.modality_x.likelihood == "gaussian"
            && self.design.modality_y.likelihood == "gaussian"
            && self.design.missingness_assumption == "complete_paired_rows"
            && self
                .design
                .coordinate_frame
                .as_ref()
                .is_none_or(|frame| !frame.trim().is_empty());
        let train_count = self.rows.iter().filter(|row| row.split == "train").count();
        let test_count = self.rows.iter().filter(|row| row.split == "test").count();
        let mut entity_ids = HashSet::new();
        if !valid_design
            || !(1..=32).contains(&dx)
            || !(1..=32).contains(&dy)
            || !(9..=10_000).contains(&self.rows.len())
            || train_count < 8
            || test_count == 0
            || !(1..=dx.min(dy)).contains(&self.latent_dimensions)
            || !self.regularization.is_finite()
            || self.regularization < 0.0
            || !self.noise_floor.is_finite()
            || self.noise_floor <= 0.0
            || !(2..=10_000).contains(&self.maximum_iterations)
            || !self.convergence_tolerance.is_finite()
            || self.convergence_tolerance <= 0.0
            || !(1..=3_600).contains(&self.timeout_seconds)
            || self.rows.iter().any(|row| {
                row.entity_id.trim().is_empty()
                    || row.entity_id.trim() != row.entity_id
                    || !entity_ids.insert(row.entity_id.as_str())
                    || !matches!(row.split.as_str(), "train" | "test")
                    || row.x.len() != dx
                    || row.y.len() != dy
                    || row.x.iter().chain(&row.y).any(|value| !value.is_finite())
            })
        {
            return Err(BayesError::InvalidSpec(
                "pCCA requires valid paired measured patient Gaussian modalities, train/test rows, dimensions, and numerical controls"
                    .into(),
            ));
        }
        for names in [
            &self.design.modality_x.feature_names,
            &self.design.modality_y.feature_names,
        ] {
            let mut unique = HashSet::new();
            if names.iter().any(|name| {
                name.trim().is_empty() || name.trim() != name || !unique.insert(name.as_str())
            }) {
                return Err(BayesError::InvalidSpec(
                    "pCCA feature names must be unique exact strings within each modality".into(),
                ));
            }
        }
        self.rows
            .sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
        Ok(self)
    }
}
