#[cfg(feature = "cli")]
use std::{fs, path::Path};

use crate::{
    common::finite::validate_serializable_finite,
    errors::{MarklabError, Result},
};

use super::result_types::{
    AnalysisResult, MarkedPatternResult, MultimodalResult, PrePostResult, Provenance,
    ResultDocument, RESULT_FORMAT_VERSION,
};

impl ResultDocument {
    pub fn marked(result: MarkedPatternResult) -> Self {
        Self::new(AnalysisResult::MarkedPattern(result))
    }

    pub fn multimodal(result: MultimodalResult) -> Self {
        Self::new(AnalysisResult::Multimodal(result))
    }

    pub fn marked_prepost(result: PrePostResult) -> Self {
        Self::new(AnalysisResult::MarkedPrePost(result))
    }

    pub fn multimodal_prepost(result: PrePostResult) -> Self {
        Self::new(AnalysisResult::MultimodalPrePost(result))
    }

    fn new(analysis: AnalysisResult) -> Self {
        Self {
            format_version: RESULT_FORMAT_VERSION.into(),
            provenance: Provenance {
                program: "marklab".into(),
                crate_version: env!("CARGO_PKG_VERSION").into(),
            },
            analysis,
        }
    }

    pub fn from_json(text: &str) -> Result<Self> {
        let mut value: serde_json::Value = serde_json::from_str(text)
            .map_err(|error| MarklabError::Schema(format!("invalid result JSON: {error}")))?;
        let found = value
            .get("format_version")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| MarklabError::Schema("result format_version is required".into()))?;
        if found == "0.2" {
            value = super::migrate_v02::marked_document(value)?;
        } else if found != RESULT_FORMAT_VERSION {
            return Err(MarklabError::UnsupportedFormatVersion {
                found: found.into(),
                supported: RESULT_FORMAT_VERSION.into(),
            });
        }
        serde_json::from_value(value)
            .map_err(|error| MarklabError::Schema(format!("invalid result document: {error}")))
    }

    pub fn into_marked_pattern(self) -> Result<MarkedPatternResult> {
        match self.analysis {
            AnalysisResult::MarkedPattern(result) => Ok(result),
            AnalysisResult::Multimodal(_)
            | AnalysisResult::MarkedPrePost(_)
            | AnalysisResult::MultimodalPrePost(_) => Err(MarklabError::Validation(
                "expected a marked_pattern result document".into(),
            )),
        }
    }

    pub fn into_multimodal(self) -> Result<MultimodalResult> {
        match self.analysis {
            AnalysisResult::Multimodal(result) => Ok(result),
            AnalysisResult::MarkedPattern(_)
            | AnalysisResult::MarkedPrePost(_)
            | AnalysisResult::MultimodalPrePost(_) => Err(MarklabError::Validation(
                "expected a multimodal result document".into(),
            )),
        }
    }

    pub fn into_marked_prepost(self) -> Result<PrePostResult> {
        match self.analysis {
            AnalysisResult::MarkedPrePost(result) => Ok(result),
            AnalysisResult::MarkedPattern(_)
            | AnalysisResult::Multimodal(_)
            | AnalysisResult::MultimodalPrePost(_) => Err(MarklabError::Validation(
                "expected a marked_prepost result document".into(),
            )),
        }
    }

    pub fn into_multimodal_prepost(self) -> Result<PrePostResult> {
        match self.analysis {
            AnalysisResult::MultimodalPrePost(result) => Ok(result),
            AnalysisResult::MarkedPattern(_)
            | AnalysisResult::Multimodal(_)
            | AnalysisResult::MarkedPrePost(_) => Err(MarklabError::Validation(
                "expected a multimodal_prepost result document".into(),
            )),
        }
    }

    pub fn to_json_pretty(&self) -> Result<String> {
        self.validated_json()
    }

    pub(super) fn validated_json(&self) -> Result<String> {
        if self.format_version != RESULT_FORMAT_VERSION {
            return Err(MarklabError::UnsupportedFormatVersion {
                found: self.format_version.clone(),
                supported: RESULT_FORMAT_VERSION.into(),
            });
        }
        validate_serializable_finite(self).map_err(|error| {
            MarklabError::Compute(format!(
                "result document contains invalid floating-point data: {error}"
            ))
        })?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|error| MarklabError::Compute(error.to_string()))?;
        serde_json::from_str::<Self>(&json).map_err(|error| {
            MarklabError::Schema(format!(
                "result document cannot be represented by format 0.3: {error}"
            ))
        })?;
        Ok(json)
    }
}

#[cfg(feature = "cli")]
pub(crate) fn read_result_document_path_or_dir(path: &Path) -> Result<ResultDocument> {
    let result_path = if path.is_dir() {
        path.join("result.json")
    } else {
        path.to_path_buf()
    };
    ResultDocument::from_json(&fs::read_to_string(result_path)?)
}
