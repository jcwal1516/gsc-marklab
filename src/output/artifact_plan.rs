use std::path::{Path, PathBuf};

use crate::{config::OutputSection, errors::Result};

use super::{
    manifest::{RunManifest, RunManifestContext},
    AnalysisResult, ResultDocument,
};

pub(super) struct ArtifactPlan<'a> {
    pub(super) document: &'a ResultDocument,
    pub(super) result_json: String,
    pub(super) run_manifest: Option<RunManifest>,
    required_artifacts: Vec<PathBuf>,
}

impl<'a> ArtifactPlan<'a> {
    pub(super) fn new(
        document: &'a ResultDocument,
        options: &OutputSection,
        manifest_context: Option<RunManifestContext>,
    ) -> Result<Self> {
        let mut required_artifacts = match &document.analysis {
            AnalysisResult::MarkedPattern(_) => vec![
                PathBuf::from("result.json"),
                PathBuf::from("qc.json"),
                PathBuf::from("report.md"),
                PathBuf::from("timings.json"),
            ],
            AnalysisResult::Multimodal(_) => vec![
                PathBuf::from("result.json"),
                PathBuf::from("registration_qc.json"),
                PathBuf::from("report.md"),
                PathBuf::from("timings.json"),
            ],
            AnalysisResult::MarkedPrePost(_) | AnalysisResult::MultimodalPrePost(_) => {
                vec![PathBuf::from("result.json")]
            }
        };
        let run_manifest = options.write_run_manifest.then(|| {
            required_artifacts.push(PathBuf::from("run_manifest.json"));
            RunManifest::from_document(document, options, manifest_context)
        });

        Ok(Self {
            document,
            result_json: document.validated_json()?,
            run_manifest,
            required_artifacts,
        })
    }

    pub(super) fn validate_staging(&self, staging: &Path) -> Result<()> {
        for relative_path in &self.required_artifacts {
            let path = staging.join(relative_path);
            if !path.is_file() {
                return Err(crate::MarklabError::Validation(format!(
                    "required output artifact was not written: {}",
                    path.display()
                )));
            }
        }
        Ok(())
    }
}
