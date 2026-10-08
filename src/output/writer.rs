use std::{collections::BTreeMap, path::Path};

use crate::{
    config::OutputSection,
    errors::{MarklabError, Result},
};

use super::artifact_io::write_json;
use super::artifact_plan::ArtifactPlan;
use super::manifest::RunManifestContext;
use super::marked_artifacts::write_core_marked_outputs;
use super::multimodal_result_artifacts::write_core_multimodal_outputs;
use super::result_types::{AnalysisResult, ArtifactStatus, OutputManifest, ResultDocument};
use super::transaction::OutputTransaction;

#[derive(Clone, Copy, Debug, Default)]
pub struct OutputWriter;

impl OutputWriter {
    pub fn write(
        document: &ResultDocument,
        out: impl AsRef<Path>,
        options: &OutputSection,
    ) -> Result<OutputManifest> {
        Self::write_transaction(document, out, options, None, |_| Ok(()))
    }

    pub(crate) fn write_transaction<F>(
        document: &ResultDocument,
        out: impl AsRef<Path>,
        options: &OutputSection,
        manifest_context: Option<RunManifestContext>,
        additional_artifacts: F,
    ) -> Result<OutputManifest>
    where
        F: FnOnce(&Path) -> Result<()>,
    {
        let out = out.as_ref();
        let plan = ArtifactPlan::new(document, options, manifest_context)?;
        let transaction = OutputTransaction::new(out)?;
        let staging = transaction.staging_path().to_path_buf();
        let mut manifest = Self::write_in_place(&plan, &staging, options)?;
        additional_artifacts(&staging)?;
        plan.validate_staging(&staging)?;
        validate_written_manifest(&manifest)?;
        transaction.commit()?;
        rebase_output_manifest(&mut manifest, &staging, out)?;
        Ok(manifest)
    }

    #[cfg(feature = "cli")]
    pub(crate) fn write_comparison_transaction<F>(
        document: &ResultDocument,
        out: impl AsRef<Path>,
        additional_artifacts: F,
    ) -> Result<()>
    where
        F: FnOnce(&Path) -> Result<()>,
    {
        if !matches!(
            document.analysis,
            AnalysisResult::MarkedPrePost(_) | AnalysisResult::MultimodalPrePost(_)
        ) {
            return Err(MarklabError::Validation(
                "comparison output requires a marked_prepost or multimodal_prepost document".into(),
            ));
        }
        let out = out.as_ref();
        let transaction = OutputTransaction::new(out)?;
        let staging = transaction.staging_path();
        let prepost_path = staging.join("prepost.json");
        std::fs::write(&prepost_path, document.validated_json()?)
            .map_err(|source| MarklabError::io(&prepost_path, source))?;
        additional_artifacts(staging)?;
        if !prepost_path.is_file() {
            return Err(MarklabError::Validation(
                "comparison transaction did not write prepost.json".into(),
            ));
        }
        transaction.commit()
    }

    fn write_in_place(
        plan: &ArtifactPlan<'_>,
        out: &Path,
        options: &OutputSection,
    ) -> Result<OutputManifest> {
        match &plan.document.analysis {
            AnalysisResult::MarkedPattern(result) => {
                write_core_marked_outputs(result, out, options)?;
            }
            AnalysisResult::Multimodal(result) => {
                write_core_multimodal_outputs(result, out, options)?;
            }
            AnalysisResult::MarkedPrePost(_) | AnalysisResult::MultimodalPrePost(_) => {
                std::fs::create_dir_all(out).map_err(|source| MarklabError::io(out, source))?;
            }
        }

        let result_path = out.join("result.json");
        std::fs::write(&result_path, &plan.result_json)
            .map_err(|source| MarklabError::io(&result_path, source))?;
        if let Some(run_manifest) = &plan.run_manifest {
            write_json(out.join("run_manifest.json"), run_manifest)?;
        }

        let mut artifacts = BTreeMap::new();
        artifacts.insert(
            "parquet_curves".into(),
            match &plan.document.analysis {
                AnalysisResult::MarkedPattern(_) => {
                    artifact_group_status(out, options.write_parquet_curves, "spectra.parquet")
                }
                AnalysisResult::Multimodal(_) => {
                    artifact_group_status(out, options.write_parquet_curves, "fused_cells.parquet")
                }
                AnalysisResult::MarkedPrePost(_) | AnalysisResult::MultimodalPrePost(_) => {
                    ArtifactStatus::NotApplicable
                }
            },
        );
        artifacts.insert(
            match &plan.document.analysis {
                AnalysisResult::MarkedPattern(_) => "residual_territories".into(),
                AnalysisResult::Multimodal(_) => "neighborhood_territories".into(),
                AnalysisResult::MarkedPrePost(_) | AnalysisResult::MultimodalPrePost(_) => {
                    "comparison_artifacts".into()
                }
            },
            match &plan.document.analysis {
                AnalysisResult::MarkedPattern(_) => artifact_group_status(
                    out,
                    options.write_geojson_territories,
                    "residual_territories.geojson",
                ),
                AnalysisResult::Multimodal(_) => artifact_group_status(
                    out,
                    options.write_geojson_territories,
                    "neighborhood_territories.geojson",
                ),
                AnalysisResult::MarkedPrePost(_) | AnalysisResult::MultimodalPrePost(_) => {
                    ArtifactStatus::NotApplicable
                }
            },
        );
        artifacts.insert(
            "figures".into(),
            match &plan.document.analysis {
                AnalysisResult::MarkedPattern(_) => {
                    artifact_group_status(out, options.write_figures, "figures/spectrum.svg")
                }
                AnalysisResult::Multimodal(_) => ArtifactStatus::NotApplicable,
                AnalysisResult::MarkedPrePost(_) | AnalysisResult::MultimodalPrePost(_) => {
                    ArtifactStatus::NotApplicable
                }
            },
        );

        Ok(OutputManifest {
            result: ArtifactStatus::Written {
                path: out.join("result.json"),
            },
            artifacts,
        })
    }
}

fn validate_written_manifest(manifest: &OutputManifest) -> Result<()> {
    validate_written_status(&manifest.result)?;
    for status in manifest.artifacts.values() {
        validate_written_status(status)?;
    }
    Ok(())
}

fn validate_written_status(status: &ArtifactStatus) -> Result<()> {
    if let ArtifactStatus::Written { path } = status {
        if !path.is_file() {
            return Err(MarklabError::Validation(format!(
                "planned output artifact was not written: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn rebase_output_manifest(
    manifest: &mut OutputManifest,
    staging: &Path,
    final_path: &Path,
) -> Result<()> {
    rebase_artifact_status(&mut manifest.result, staging, final_path)?;
    for status in manifest.artifacts.values_mut() {
        rebase_artifact_status(status, staging, final_path)?;
    }
    Ok(())
}

fn rebase_artifact_status(
    status: &mut ArtifactStatus,
    staging: &Path,
    final_path: &Path,
) -> Result<()> {
    if let ArtifactStatus::Written { path } = status {
        let relative = path.strip_prefix(staging).map_err(|_| {
            MarklabError::Compute(format!(
                "written artifact path is outside the output transaction: {}",
                path.display()
            ))
        })?;
        *path = final_path.join(relative);
    }
    Ok(())
}

fn artifact_group_status(out: &Path, enabled: bool, representative: &str) -> ArtifactStatus {
    if !enabled {
        ArtifactStatus::Disabled
    } else {
        let path = out.join(representative);
        if path.exists() {
            ArtifactStatus::Written { path }
        } else {
            ArtifactStatus::InsufficientData {
                reason: "no available data produced this artifact".into(),
            }
        }
    }
}
