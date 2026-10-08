use serde::{Deserialize, Serialize};

use super::{marked::MarkedPatternResult, multimodal::MultimodalResult, prepost::PrePostResult};

pub const RESULT_FORMAT_VERSION: &str = "0.3";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    Ok,
    Suppressed,
}

impl AnalysisStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Suppressed => "suppressed",
        }
    }
}

impl std::fmt::Display for AnalysisStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationClass {
    MultimodalSummary,
    SeparateComponents,
    SuppressedQcArtifact,
    Suppressed,
    InsufficientData,
    CoarseExcess,
    LowFrequencySuppression,
    RandomLike,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResultDocument {
    pub format_version: String,
    pub provenance: Provenance,
    pub analysis: AnalysisResult,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub program: String,
    pub crate_version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", content = "result", rename_all = "snake_case")]
// The unboxed variants are part of the result-format 0.3 Rust contract.
#[allow(clippy::large_enum_variant)]
pub enum AnalysisResult {
    MarkedPattern(MarkedPatternResult),
    Multimodal(MultimodalResult),
    #[serde(rename = "marked_prepost")]
    MarkedPrePost(PrePostResult),
    #[serde(rename = "multimodal_prepost")]
    MultimodalPrePost(PrePostResult),
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AnalysisSection<T> {
    Available {
        value: T,
    },
    Disabled,
    #[default]
    NotApplicable,
    InsufficientData {
        reason: String,
    },
}

impl<T> AnalysisSection<T> {
    pub fn available(value: T) -> Self {
        Self::Available { value }
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Available { value } => Some(value),
            Self::Disabled | Self::NotApplicable | Self::InsufficientData { .. } => None,
        }
    }

    pub fn value_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Available { value } => Some(value),
            Self::Disabled | Self::NotApplicable | Self::InsufficientData { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TimingStage {
    pub stage_name: String,
    pub wall_ms: f64,
    pub cpu_threads: usize,
    pub n_cells: usize,
    pub n_marked: usize,
    pub n_k_modes: usize,
    pub n_permutations: usize,
    pub estimated_peak_memory_mib: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Interpretation {
    pub class: InterpretationClass,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum StatusFlag {
    UnderpoweredTooFewCells,
    UnderpoweredTooFewMarked,
    UnderpoweredTooFewUnmarked,
    UnderpoweredAreaTooSmall,
    UnderpoweredTooFewKShells,
    InvalidIhcMask,
    InternalControlFailureOverlap,
    StainGradientSuspect,
    MaskFragmentationSuspect,
    WindowOrGriddingArtifactSuspect,
    SensitivityUnstable,
    ConfoundedBySpatialStrata,
    DegenerateSpatialStrataNull,
    PrePostNotAnatomicallyComparable,
    SuppressedBiologicInterpretation,
}

pub(super) const fn default_true() -> bool {
    true
}
