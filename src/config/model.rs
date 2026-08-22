use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisConfig {
    pub analysis: AnalysisConfigSection,
    pub validation: ValidationSection,
    pub spectrum: SpectrumSection,
    pub periodogram: PeriodogramSection,
    pub multiscale_residual: MultiscaleResidualSection,
    pub permutation: PermutationSection,
    pub inference: InferenceSection,
    #[serde(default)]
    pub diagnostics: DiagnosticsSection,
    #[serde(default)]
    pub registration: RegistrationSection,
    #[serde(default)]
    pub neighborhood: NeighborhoodSection,
    #[serde(default)]
    pub comparison: ComparisonSection,
    pub performance: PerformanceSection,
    pub output: OutputSection,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisConfigSection {
    pub mark_label: String,
    pub use_probabilistic_marks: bool,
    pub analyze_components: ComponentMode,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComponentMode {
    Auto,
    Pooled,
    Separate,
    Both,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationSection {
    pub n_min: usize,
    pub n_marked_min: usize,
    pub n_unmarked_min: usize,
    pub p_min: f64,
    pub p_max: f64,
    pub area_min_um2: f64,
    pub k_shell_min: usize,
    pub largest_interpretable_scale_fraction: f64,
    pub valid_mask_fraction_min: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpectrumSection {
    pub k_shells: usize,
    pub low_k_shells: usize,
    pub fit_low_k_alpha: bool,
    pub anisotropy_low_k_shells: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PeriodogramSection {
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MultiscaleResidualSection {
    pub enabled: bool,
    pub territory_detection: bool,
    pub min_territory_z: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PermutationSection {
    pub b: usize,
    pub seed: u64,
    pub stratified: bool,
    pub strata_fields: Vec<PermutationStratum>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PermutationStratum {
    QcBin,
    ComponentId,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InferenceSection {
    pub family_wise_alpha: f64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticsSection {
    pub beta_posterior_groups: bool,
    pub graph_smoothing: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationSection {
    pub enabled: bool,
    pub transform: RegistrationTransform,
    pub min_landmarks: usize,
    pub max_rmse_um: f64,
    pub claim_distance_multiplier: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationTransform {
    /// Six-parameter affine fit; permits scale, shear, rotation, and translation.
    Affine,
    /// Orientation-preserving rotation and translation with no scale or reflection.
    Rigid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NeighborhoodSection {
    pub enabled: bool,
    pub radius_um: f64,
    pub k_nearest: usize,
    pub label_pairs: Vec<[String; 2]>,
    pub territory_eps_um: f64,
    pub territory_min_cells: usize,
    pub territory_min_radius_um: f64,
    pub null_models: Vec<NeighborhoodNullModel>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum NeighborhoodNullModel {
    SourceSection,
    SourceSectionDensity,
    SourceSectionCellClass,
    SourceSectionRegistrationQc,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonSection {
    pub margins: CurveMargins,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CurveMargins {
    pub spectrum: Option<f64>,
    pub mark_pair_covariance: Option<f64>,
    pub cross_interaction: Option<f64>,
    pub graph_enrichment_log2: Option<f64>,
    pub territory_profile: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceSection {
    pub threads: ThreadSetting,
    pub memory_budget_mib: usize,
    pub k_chunk_modes: usize,
    pub strict_repro: bool,
    pub save_intermediates: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadSetting {
    Auto,
    Count(usize),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ThreadSettingRepr {
    Keyword(ThreadKeyword),
    Count(usize),
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ThreadKeyword {
    Auto,
}

impl<'de> Deserialize<'de> for ThreadSetting {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match ThreadSettingRepr::deserialize(deserializer)? {
            ThreadSettingRepr::Keyword(ThreadKeyword::Auto) => Self::Auto,
            ThreadSettingRepr::Count(count) => Self::Count(count),
        })
    }
}

impl Serialize for ThreadSetting {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::Count(count) => serializer.serialize_u64(*count as u64),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OutputSection {
    pub write_parquet_curves: bool,
    pub write_geojson_territories: bool,
    pub write_figures: bool,
    pub write_run_manifest: bool,
}
