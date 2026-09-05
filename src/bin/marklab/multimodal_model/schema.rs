use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModalityDesign {
    pub(super) id: String,
    pub(super) measurement_status: String,
    pub(super) likelihood: String,
    pub(super) feature_names: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PairedMultimodalDesign {
    pub(super) entity_level: String,
    pub(super) modality_x: ModalityDesign,
    pub(super) modality_y: ModalityDesign,
    pub(super) missingness_assumption: String,
    pub(super) coordinate_frame: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PairedMultimodalRow {
    pub(super) entity_id: String,
    pub(super) split: String,
    pub(super) x: Vec<f64>,
    pub(super) y: Vec<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BayesianPccaSpec {
    pub(super) design: PairedMultimodalDesign,
    pub(super) rows: Vec<PairedMultimodalRow>,
    pub(super) latent_dimensions: usize,
    pub(super) priors: String,
    pub(super) warmup: usize,
    pub(super) samples: usize,
    pub(super) target_accept: f64,
    pub(super) seed: u64,
    pub(super) timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MultiviewDesign {
    pub(super) entity_level: String,
    pub(super) modalities: Vec<ModalityDesign>,
    pub(super) missingness_assumption: String,
    pub(super) coordinate_frame: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MultiviewValues {
    pub(super) values: Vec<f64>,
    pub(super) observed: Vec<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MultiviewRow {
    pub(super) entity_id: String,
    pub(super) split: String,
    pub(super) views: Vec<MultiviewValues>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MofaSpec {
    pub(super) design: MultiviewDesign,
    pub(super) rows: Vec<MultiviewRow>,
    pub(super) maximum_factors: usize,
    pub(super) iterations: usize,
    pub(super) convergence_mode: String,
    pub(super) seed: u64,
    pub(super) timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MatrixFactorRow {
    pub(super) entity_id: String,
    pub(super) values: Vec<f64>,
    pub(super) observed: Vec<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MatrixFactorSpec {
    pub(super) matrix_id: String,
    pub(super) entity_level: String,
    pub(super) likelihood: String,
    pub(super) feature_names: Vec<String>,
    pub(super) rows: Vec<MatrixFactorRow>,
    pub(super) factors: usize,
    pub(super) iterations: usize,
    pub(super) convergence_mode: String,
    pub(super) seed: u64,
    pub(super) timeout_seconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HierarchicalEntity {
    pub(super) entity_id: String,
    pub(super) level: String,
    pub(super) parent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HierarchicalModality {
    pub(super) modality_id: String,
    pub(super) entity_level: String,
    pub(super) entity_ids: Vec<String>,
    pub(super) measurement_status: String,
    pub(super) likelihood: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HierarchicalFactorSpec {
    pub(super) model_id: String,
    pub(super) latent_dimensions: usize,
    pub(super) entities: Vec<HierarchicalEntity>,
    pub(super) modalities: Vec<HierarchicalModality>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SpatialGraphEdge {
    pub(super) left: String,
    pub(super) right: String,
    pub(super) weight: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SpatialFactorGraph {
    pub(super) graph_id: String,
    pub(super) edges: Vec<SpatialGraphEdge>,
    pub(super) normalization: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SpatialMatrixFactorSpec {
    pub(super) matrix_id: String,
    pub(super) entity_level: String,
    pub(super) likelihood: String,
    pub(super) feature_names: Vec<String>,
    pub(super) rows: Vec<MatrixFactorRow>,
    pub(super) graph: SpatialFactorGraph,
    pub(super) factors: usize,
    pub(super) spatial_precision: f64,
    pub(super) diagonal_epsilon: f64,
    pub(super) loading_precision: f64,
    pub(super) noise_standard_deviation: f64,
    pub(super) maximum_iterations: usize,
    pub(super) seed: u64,
    pub(super) timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TensorEntry {
    pub(super) indices: Vec<usize>,
    pub(super) value: f64,
    pub(super) observed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TensorFactorSpec {
    pub(super) tensor_id: String,
    pub(super) mode_names: Vec<String>,
    pub(super) shape: Vec<usize>,
    pub(super) entries: Vec<TensorEntry>,
    pub(super) decomposition: String,
    pub(super) ranks: Vec<usize>,
    pub(super) prior_precision: f64,
    pub(super) noise_standard_deviation: f64,
    pub(super) maximum_iterations: usize,
    pub(super) seed: u64,
    pub(super) timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SpatialLatentRow {
    pub(super) entity_id: String,
    pub(super) coordinates_um: [f64; 2],
    pub(super) values: Vec<f64>,
    pub(super) observed: Vec<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SpatialLatentFactorSpec {
    pub(super) matrix_id: String,
    pub(super) entity_level: String,
    pub(super) coordinate_frame: String,
    pub(super) likelihood: String,
    pub(super) feature_names: Vec<String>,
    pub(super) rows: Vec<SpatialLatentRow>,
    pub(super) factors: usize,
    pub(super) matern_nu: f64,
    pub(super) length_scale_prior_um: f64,
    pub(super) noise_standard_deviation: f64,
    pub(super) warmup: usize,
    pub(super) samples: usize,
    pub(super) target_accept: f64,
    pub(super) seed: u64,
    pub(super) timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MultiresolutionScale {
    pub(super) scale_id: String,
    pub(super) physical_scale_um: f64,
    pub(super) basis_columns: Vec<Vec<f64>>,
    pub(super) factors: usize,
    pub(super) coefficient_precision: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MultiresolutionFactorSpec {
    pub(super) matrix_id: String,
    pub(super) entity_level: String,
    pub(super) likelihood: String,
    pub(super) feature_names: Vec<String>,
    pub(super) rows: Vec<MatrixFactorRow>,
    pub(super) scales: Vec<MultiresolutionScale>,
    pub(super) loading_precision: f64,
    pub(super) noise_standard_deviation: f64,
    pub(super) maximum_iterations: usize,
    pub(super) seed: u64,
    pub(super) timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DropoutView {
    pub(super) values: Vec<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DropoutRow {
    pub(super) entity_id: String,
    pub(super) split: String,
    pub(super) views: Vec<DropoutView>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DropoutPattern {
    pub(super) pattern_id: String,
    pub(super) retained_modalities: Vec<String>,
    pub(super) probability: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DropoutRobustSpec {
    pub(super) design: MultiviewDesign,
    pub(super) rows: Vec<DropoutRow>,
    pub(super) latent_dimensions: usize,
    pub(super) dropout_patterns: Vec<DropoutPattern>,
    pub(super) required_anchor_modalities: Vec<String>,
    pub(super) consistency_weight: f64,
    pub(super) parameter_precision: f64,
    pub(super) maximum_iterations: usize,
    pub(super) seed: u64,
    pub(super) timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JointRegion {
    pub(super) region_id: String,
    pub(super) patient_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JointRegionObservation {
    pub(super) region_id: String,
    pub(super) morphology: Vec<f64>,
    pub(super) ihc: Vec<f64>,
    pub(super) omics_counts: Vec<u64>,
    pub(super) library_size: f64,
    pub(super) clone_label: u8,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JointPatientOutcome {
    pub(super) patient_id: String,
    pub(super) value: f64,
    pub(super) observed: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JointObservationBlock {
    pub(super) measurement_status: String,
    pub(super) likelihood: String,
    pub(super) feature_names: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JointModelSpec {
    pub(super) morphology: JointObservationBlock,
    pub(super) ihc: JointObservationBlock,
    pub(super) omics: JointObservationBlock,
    pub(super) clone: JointObservationBlock,
    pub(super) clinical: JointObservationBlock,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JointPathologySpec {
    pub(super) project_id: String,
    pub(super) patients: Vec<String>,
    pub(super) regions: Vec<JointRegion>,
    pub(super) region_observations: Vec<JointRegionObservation>,
    pub(super) patient_outcomes: Vec<JointPatientOutcome>,
    pub(super) model_spec: JointModelSpec,
    pub(super) inference_plan: String,
    pub(super) region_latent_standard_deviation: f64,
    pub(super) gaussian_noise_standard_deviation: f64,
    pub(super) parameter_precision: f64,
    pub(super) maximum_iterations: usize,
    pub(super) seed: u64,
    pub(super) timeout_seconds: u64,
}
