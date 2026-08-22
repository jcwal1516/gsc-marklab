use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CellSection {
    He,
    Ihc,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct HeCell {
    pub cell_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub cell_type: Option<String>,
    pub cell_type_probability: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct IhcCell {
    pub cell_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub mmr_mark: Option<u8>,
    pub mmr_probability: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AnalysisMetadata {
    pub case_id: String,
    pub timepoint: String,
    pub protein: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct FusedCell {
    pub source_section: CellSection,
    pub source_cell_id: String,
    pub x_um_registered: f64,
    pub y_um_registered: f64,
    pub mmr_mark: Option<u8>,
    pub mmr_probability: Option<f64>,
    pub cell_type: Option<String>,
    pub cell_type_probability: Option<f64>,
    pub same_section: bool,
    pub registration_error_um: Option<f64>,
}
