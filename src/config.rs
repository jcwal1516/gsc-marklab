use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::errors::{MarklabError, Result};

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
    pub beta_binomial: bool,
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

impl Default for RegistrationSection {
    fn default() -> Self {
        Self {
            enabled: true,
            transform: RegistrationTransform::Affine,
            min_landmarks: 6,
            max_rmse_um: 25.0,
            claim_distance_multiplier: 2.0,
        }
    }
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

impl Default for NeighborhoodSection {
    fn default() -> Self {
        Self {
            enabled: true,
            radius_um: 50.0,
            k_nearest: 8,
            label_pairs: vec![
                ["mmr_abnormal".into(), "mmr_abnormal".into()],
                ["mmr_abnormal".into(), "lymphocyte".into()],
            ],
            territory_eps_um: 50.0,
            territory_min_cells: 1,
            territory_min_radius_um: 1.0,
            null_models: vec![
                NeighborhoodNullModel::SourceSection,
                NeighborhoodNullModel::SourceSectionDensity,
                NeighborhoodNullModel::SourceSectionCellClass,
                NeighborhoodNullModel::SourceSectionRegistrationQc,
            ],
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonSection {
    pub equivalence_margins: EquivalenceMargins,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EquivalenceMargins {
    pub spectrum: Option<f64>,
    pub pair_correlation: Option<f64>,
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

impl Default for OutputSection {
    fn default() -> Self {
        Self {
            write_parquet_curves: true,
            write_geojson_territories: true,
            write_figures: true,
            write_run_manifest: true,
        }
    }
}

impl AnalysisConfig {
    pub fn from_toml_path(path: impl AsRef<Path>) -> Result<Self> {
        let path_ref = path.as_ref();
        let text = std::fs::read_to_string(path_ref)
            .map_err(|source| MarklabError::io(path_ref, source))?;
        deserialize_toml(&text).and_then(Self::validated)
    }

    pub fn from_toml_overrides(text: &str) -> Result<Self> {
        if text.trim().is_empty() {
            return Self::default().validated();
        }

        let default_text = toml::to_string(&Self::default())
            .map_err(|err| MarklabError::Config(err.to_string()))?;
        let mut merged = default_text
            .parse::<toml::Value>()
            .map_err(|err| MarklabError::Config(err.to_string()))?;
        let overrides = text
            .parse::<toml::Value>()
            .map_err(|err| MarklabError::Config(err.to_string()))?;
        merge_toml_value(&mut merged, overrides);
        let merged_text =
            toml::to_string(&merged).map_err(|err| MarklabError::Config(err.to_string()))?;
        deserialize_toml(&merged_text).and_then(Self::validated)
    }

    pub fn validate(&self) -> Result<()> {
        if self.analysis.mark_label.trim().is_empty() {
            return config_error("analysis.mark_label must not be empty");
        }
        if self.validation.n_min == 0 {
            return config_error("validation.n_min must be greater than zero");
        }
        if self.validation.n_marked_min + self.validation.n_unmarked_min > self.validation.n_min {
            return config_error(
                "validation.n_marked_min + validation.n_unmarked_min must not exceed validation.n_min",
            );
        }
        if !unit_interval_open(self.validation.p_min)
            || !unit_interval_open(self.validation.p_max)
            || self.validation.p_min >= self.validation.p_max
        {
            return config_error(
                "validation.p_min and validation.p_max must be finite, inside (0, 1), and p_min < p_max",
            );
        }
        positive_finite("validation.area_min_um2", self.validation.area_min_um2)?;
        if self.validation.k_shell_min == 0 {
            return config_error("validation.k_shell_min must be greater than zero");
        }
        if !unit_interval_open(self.validation.largest_interpretable_scale_fraction) {
            return config_error(
                "validation.largest_interpretable_scale_fraction must be finite and inside (0, 1)",
            );
        }
        if !unit_interval_closed(self.validation.valid_mask_fraction_min) {
            return config_error(
                "validation.valid_mask_fraction_min must be finite and inside (0, 1]",
            );
        }
        if self.spectrum.k_shells == 0
            || self.spectrum.low_k_shells == 0
            || self.spectrum.anisotropy_low_k_shells == 0
        {
            return config_error("spectrum shell counts must be greater than zero");
        }
        if self.spectrum.low_k_shells > self.spectrum.k_shells
            || self.spectrum.anisotropy_low_k_shells > self.spectrum.k_shells
            || self.validation.k_shell_min > self.spectrum.k_shells
        {
            return config_error("spectrum shell subsets must not exceed spectrum.k_shells");
        }
        if self.multiscale_residual.enabled {
            positive_finite(
                "multiscale_residual.min_territory_z",
                self.multiscale_residual.min_territory_z,
            )?;
        }
        if !unit_interval_open(self.inference.family_wise_alpha) {
            return config_error("inference.family_wise_alpha must be finite and inside (0, 1)");
        }
        let n_curves = self.permutation.b.saturating_add(1);
        if n_curves as f64 * self.inference.family_wise_alpha < 1.0 {
            return config_error("permutation requires (B + 1) * alpha >= 1");
        }
        if self.spectrum.fit_low_k_alpha
            && (n_curves as f64) < 2.0 / self.inference.family_wise_alpha
        {
            return config_error("equal-tail endpoints require B + 1 >= 2 / alpha");
        }
        if self.permutation.stratified && self.permutation.strata_fields.is_empty() {
            return config_error(
                "permutation.strata_fields must not be empty when stratified is true",
            );
        }
        if self.analysis.use_probabilistic_marks && self.permutation.stratified {
            return config_error(
                "analysis.use_probabilistic_marks is not supported with stratified permutation",
            );
        }
        reject_duplicates("permutation.strata_fields", &self.permutation.strata_fields)?;

        if self.registration.enabled {
            if self.registration.min_landmarks == 0 {
                return config_error("registration.min_landmarks must be greater than zero");
            }
            nonnegative_finite("registration.max_rmse_um", self.registration.max_rmse_um)?;
            positive_finite(
                "registration.claim_distance_multiplier",
                self.registration.claim_distance_multiplier,
            )?;
        }
        if self.neighborhood.enabled {
            positive_finite("neighborhood.radius_um", self.neighborhood.radius_um)?;
            positive_finite(
                "neighborhood.territory_eps_um",
                self.neighborhood.territory_eps_um,
            )?;
            if self.neighborhood.territory_min_cells == 0 {
                return config_error("neighborhood.territory_min_cells must be greater than zero");
            }
            positive_finite(
                "neighborhood.territory_min_radius_um",
                self.neighborhood.territory_min_radius_um,
            )?;
            reject_duplicates("neighborhood.null_models", &self.neighborhood.null_models)?;
        }
        for (field, margin) in [
            ("spectrum", self.comparison.equivalence_margins.spectrum),
            (
                "pair_correlation",
                self.comparison.equivalence_margins.pair_correlation,
            ),
            (
                "cross_interaction",
                self.comparison.equivalence_margins.cross_interaction,
            ),
            (
                "graph_enrichment_log2",
                self.comparison.equivalence_margins.graph_enrichment_log2,
            ),
            (
                "territory_profile",
                self.comparison.equivalence_margins.territory_profile,
            ),
        ] {
            if let Some(value) = margin {
                positive_finite(&format!("comparison.equivalence_margins.{field}"), value)?;
            }
        }
        if matches!(self.performance.threads, ThreadSetting::Count(0)) {
            return config_error("performance.threads must be 'auto' or a positive integer");
        }
        if self.performance.memory_budget_mib == 0 || self.performance.k_chunk_modes == 0 {
            return config_error(
                "performance.memory_budget_mib and performance.k_chunk_modes must be greater than zero",
            );
        }
        Ok(())
    }

    fn validated(self) -> Result<Self> {
        self.validate()?;
        Ok(self)
    }
}

fn deserialize_toml(text: &str) -> Result<AnalysisConfig> {
    let deserializer = toml::de::Deserializer::new(text);
    serde_path_to_error::deserialize(deserializer).map_err(|error| {
        let path = error.path().to_string();
        let detail = error.inner();
        if path.is_empty() {
            MarklabError::Config(detail.to_string())
        } else {
            MarklabError::Config(format!("{path}: {detail}"))
        }
    })
}

fn merge_toml_value(target: &mut toml::Value, source: toml::Value) {
    match (target, source) {
        (toml::Value::Table(target), toml::Value::Table(source)) => {
            for (key, value) in source {
                if let Some(existing) = target.get_mut(&key) {
                    merge_toml_value(existing, value);
                } else {
                    target.insert(key, value);
                }
            }
        }
        (target, source) => *target = source,
    }
}

fn config_error<T>(message: impl Into<String>) -> Result<T> {
    Err(MarklabError::Config(message.into()))
}

fn positive_finite(field: &str, value: f64) -> Result<()> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        config_error(format!("{field} must be finite and positive"))
    }
}

fn nonnegative_finite(field: &str, value: f64) -> Result<()> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        config_error(format!("{field} must be finite and non-negative"))
    }
}

fn unit_interval_open(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value < 1.0
}

fn unit_interval_closed(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value <= 1.0
}

fn reject_duplicates<T: Ord>(field: &str, values: &[T]) -> Result<()> {
    let mut unique = BTreeSet::new();
    if values.iter().any(|value| !unique.insert(value)) {
        config_error(format!("{field} must not contain duplicates"))
    } else {
        Ok(())
    }
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            analysis: AnalysisConfigSection {
                mark_label: "marked".into(),
                use_probabilistic_marks: false,
                analyze_components: ComponentMode::Auto,
            },
            validation: ValidationSection {
                n_min: 200,
                n_marked_min: 25,
                n_unmarked_min: 25,
                p_min: 0.02,
                p_max: 0.98,
                area_min_um2: 100_000.0,
                k_shell_min: 5,
                largest_interpretable_scale_fraction: 0.33,
                valid_mask_fraction_min: 0.5,
            },
            spectrum: SpectrumSection {
                k_shells: 64,
                low_k_shells: 3,
                fit_low_k_alpha: true,
                anisotropy_low_k_shells: 5,
            },
            periodogram: PeriodogramSection { enabled: true },
            multiscale_residual: MultiscaleResidualSection {
                enabled: true,
                territory_detection: true,
                min_territory_z: 2.5,
            },
            permutation: PermutationSection {
                b: 999,
                seed: 123_456_789,
                stratified: true,
                strata_fields: vec![PermutationStratum::QcBin, PermutationStratum::ComponentId],
            },
            inference: InferenceSection {
                family_wise_alpha: 0.05,
            },
            diagnostics: DiagnosticsSection::default(),
            registration: RegistrationSection::default(),
            neighborhood: NeighborhoodSection::default(),
            comparison: ComparisonSection::default(),
            performance: PerformanceSection {
                threads: ThreadSetting::Auto,
                memory_budget_mib: 4096,
                k_chunk_modes: 256,
                strict_repro: false,
                save_intermediates: false,
            },
            output: OutputSection::default(),
        }
    }
}
