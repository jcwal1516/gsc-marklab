use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Instant,
};

use serde::Serialize;

use crate::{
    config::OutputSection,
    errors::{MarklabError, Result},
};

use super::result_types::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct OutputWriter;

impl ResultDocument {
    pub fn marked(result: MarkedPatternResult) -> Self {
        Self::new(AnalysisResult::MarkedPattern(result))
    }

    pub fn multimodal(result: MultimodalResult) -> Self {
        Self::new(AnalysisResult::Multimodal(result))
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
        let value: serde_json::Value = serde_json::from_str(text)
            .map_err(|error| MarklabError::Schema(format!("invalid result JSON: {error}")))?;
        let found = value
            .get("format_version")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| MarklabError::Schema("result format_version is required".into()))?;
        if found != RESULT_FORMAT_VERSION {
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
            AnalysisResult::Multimodal(_) => Err(MarklabError::Validation(
                "expected a marked_pattern result document".into(),
            )),
        }
    }

    pub fn into_multimodal(self) -> Result<MultimodalResult> {
        match self.analysis {
            AnalysisResult::Multimodal(result) => Ok(result),
            AnalysisResult::MarkedPattern(_) => Err(MarklabError::Validation(
                "expected a multimodal result document".into(),
            )),
        }
    }

    fn validated_json(&self) -> Result<String> {
        if self.format_version != RESULT_FORMAT_VERSION {
            return Err(MarklabError::UnsupportedFormatVersion {
                found: self.format_version.clone(),
                supported: RESULT_FORMAT_VERSION.into(),
            });
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|error| MarklabError::Compute(error.to_string()))?;
        serde_json::from_str::<Self>(&json).map_err(|error| {
            MarklabError::Schema(format!(
                "result document cannot be represented by format 0.2: {error}"
            ))
        })?;
        Ok(json)
    }
}

fn write_marked_outputs(
    result: &MarkedPatternResult,
    out: &Path,
    options: &OutputSection,
) -> Result<()> {
    let write_start = Instant::now();
    std::fs::create_dir_all(out).map_err(|source| MarklabError::io(out, source))?;

    #[cfg(not(feature = "parquet"))]
    if options.write_parquet_curves {
        return Err(MarklabError::Config(
            "Parquet curve output requires the parquet feature".into(),
        ));
    }

    if options.write_run_manifest {
        let manifest = serde_json::json!({
            "program": "marklab",
            "crate_version": env!("CARGO_PKG_VERSION"),
            "format_version": RESULT_FORMAT_VERSION,
            "result": {
                "case_id": result.case_id,
                "timepoint": result.timepoint,
                "protein": result.protein,
                "mark_label": result.mark_label,
                "status": result.status,
                "status_flags": result.status_flags,
                "n_cells": result.n_cells,
                "n_marked": result.n_marked,
                "p_hat": result.p_hat,
            },
            "output": {
                "write_parquet_curves": options.write_parquet_curves,
                "write_geojson_territories": options.write_geojson_territories,
                "write_figures": options.write_figures,
                "write_run_manifest": options.write_run_manifest,
            },
            "timings_stage_count": result.timings.len(),
        });
        std::fs::write(
            out.join("run_manifest.json"),
            serde_json::to_string_pretty(&manifest)
                .map_err(|err| MarklabError::Compute(err.to_string()))?,
        )
        .map_err(|source| MarklabError::io(out.join("run_manifest.json"), source))?;
    }

    let qc = serde_json::json!({
        "status": result.status,
        "status_flags": result.status_flags,
        "metrics": result.qc,
    });
    std::fs::write(out.join("qc.json"), qc.to_string())
        .map_err(|source| MarklabError::io(out.join("qc.json"), source))?;

    let report = crate::io::report::render_analysis_report(result);
    std::fs::write(out.join("report.md"), report)
        .map_err(|source| MarklabError::io(out.join("report.md"), source))?;

    #[cfg(feature = "parquet")]
    if options.write_parquet_curves {
        result.write_spectra_parquet(out)?;
    }
    #[cfg(feature = "parquet")]
    if options.write_parquet_curves {
        result.write_pair_correlation_parquet(out)?;
    }
    #[cfg(feature = "parquet")]
    if options.write_parquet_curves {
        result.write_scalogram_parquet(out)?;
    }
    if options.write_geojson_territories {
        if let Some(territories) = result.wavelet_territories.value() {
            crate::io::geojson::write_territory_features(
                territories,
                out.join("wavelet_territories.geojson"),
            )?;
        }
    }
    if options.write_figures {
        super::figures::write(result, out)?;
    }

    let mut timings = result.timings.clone();
    timings.push(TimingStage {
        stage_name: "write_outputs".into(),
        wall_ms: write_start.elapsed().as_secs_f64() * 1000.0,
        cpu_threads: 1,
        n_cells: result.n_cells,
        n_marked: result.n_marked,
        n_k_modes: result.spectrum.value().map_or(0, |value| value.n_k_modes),
        n_permutations: result
            .spectrum
            .value()
            .map_or(0, |value| value.n_permutations),
        estimated_peak_memory_mib: result
            .timings
            .first()
            .map(|stage| stage.estimated_peak_memory_mib)
            .unwrap_or(0.0),
    });
    let timings_json = serde_json::json!({
        "stages": timings,
    });
    std::fs::write(out.join("timings.json"), timings_json.to_string())
        .map_err(|source| MarklabError::io(out.join("timings.json"), source))?;

    Ok(())
}

impl OutputWriter {
    pub fn write(
        document: &ResultDocument,
        out: impl AsRef<Path>,
        options: &OutputSection,
    ) -> Result<OutputManifest> {
        let out = out.as_ref();
        let result_json = document.validated_json()?;
        match &document.analysis {
            AnalysisResult::MarkedPattern(result) => {
                write_marked_outputs(result, out, options)?;
            }
            AnalysisResult::Multimodal(result) => {
                write_multimodal_outputs(result, out, options)?;
            }
        }

        let result_path = out.join("result.json");
        std::fs::write(&result_path, result_json)
            .map_err(|source| MarklabError::io(&result_path, source))?;

        let mut artifacts = BTreeMap::new();
        artifacts.insert(
            "parquet_curves".into(),
            match &document.analysis {
                AnalysisResult::MarkedPattern(_) => {
                    artifact_group_status(out, options.write_parquet_curves, "spectra.parquet")
                }
                AnalysisResult::Multimodal(_) => {
                    artifact_group_status(out, options.write_parquet_curves, "fused_cells.parquet")
                }
            },
        );
        artifacts.insert(
            match &document.analysis {
                AnalysisResult::MarkedPattern(_) => "wavelet_territories".into(),
                AnalysisResult::Multimodal(_) => "neighborhood_territories".into(),
            },
            match &document.analysis {
                AnalysisResult::MarkedPattern(_) => artifact_group_status(
                    out,
                    options.write_geojson_territories,
                    "wavelet_territories.geojson",
                ),
                AnalysisResult::Multimodal(_) => artifact_group_status(
                    out,
                    options.write_geojson_territories,
                    "neighborhood_territories.geojson",
                ),
            },
        );
        artifacts.insert(
            "figures".into(),
            match &document.analysis {
                AnalysisResult::MarkedPattern(_) => {
                    artifact_group_status(out, options.write_figures, "figures/spectrum.svg")
                }
                AnalysisResult::Multimodal(_) => ArtifactStatus::NotApplicable,
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

fn write_multimodal_outputs(
    result: &MultimodalResult,
    out: &Path,
    options: &OutputSection,
) -> Result<()> {
    std::fs::create_dir_all(out).map_err(|source| MarklabError::io(out, source))?;

    #[cfg(not(feature = "parquet"))]
    if options.write_parquet_curves {
        return Err(MarklabError::Config(
            "multimodal Parquet output requires the parquet feature".into(),
        ));
    }

    write_json(out.join("registration_qc.json"), &result.registration)?;
    write_available_json(
        out.join("neighborhood_enrichment.json"),
        &result.neighborhood_enrichment,
    )?;
    write_available_json(
        out.join("cross_interaction_curves.json"),
        &result.cross_interaction_curves,
    )?;
    write_available_json(
        out.join("neighborhood_territories.json"),
        &result.neighborhood_territories,
    )?;
    write_available_json(
        out.join("territory_profiles.json"),
        &result.territory_profiles,
    )?;
    write_available_json(
        out.join("territory_comparisons.json"),
        &result.territory_comparisons,
    )?;

    let report_path = out.join("report.md");
    std::fs::write(
        &report_path,
        crate::io::report::render_multimodal_report(result),
    )
    .map_err(|source| MarklabError::io(&report_path, source))?;

    if options.write_geojson_territories {
        if let Some(territories) = result
            .neighborhood_territories
            .value()
            .filter(|value| !value.is_empty())
        {
            crate::io::geojson::write_territory_features(
                territories,
                out.join("neighborhood_territories.geojson"),
            )?;
        }
    }

    #[cfg(feature = "parquet")]
    if options.write_parquet_curves {
        if !result.fused_cells.is_empty() {
            crate::io::parquet::write_fused_cells_parquet(
                &result.fused_cells,
                out.join("fused_cells.parquet"),
            )?;
        }
        if let Some(enrichment) = result
            .neighborhood_enrichment
            .value()
            .filter(|value| !value.is_empty())
        {
            crate::io::parquet::write_neighborhood_enrichment_parquet(
                enrichment,
                out.join("neighborhood_enrichment.parquet"),
            )?;
        }
        if let Some(curves) = result
            .cross_interaction_curves
            .value()
            .filter(|value| !value.is_empty())
        {
            crate::io::parquet::write_cross_interaction_curves_parquet(
                curves,
                out.join("cross_interaction_curves.parquet"),
            )?;
        }
    }

    if options.write_run_manifest {
        write_json(
            out.join("run_manifest.json"),
            &serde_json::json!({
                "program": "marklab",
                "crate_version": env!("CARGO_PKG_VERSION"),
                "format_version": RESULT_FORMAT_VERSION,
                "analysis_kind": "multimodal",
                "case_id": result.case_id,
                "timepoint": result.timepoint,
                "protein": result.protein,
                "status": result.status,
            }),
        )?;
    }

    if !result.timings.is_empty() {
        write_json(
            out.join("timings.json"),
            &serde_json::json!({"stages": result.timings}),
        )?;
    }
    Ok(())
}

fn write_available_json<T: Serialize>(
    path: PathBuf,
    section: &AnalysisSection<Vec<T>>,
) -> Result<()> {
    if section.value().is_some_and(|value| !value.is_empty()) {
        write_json(path, section)?;
    }
    Ok(())
}

fn write_json(path: PathBuf, value: &impl Serialize) -> Result<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| MarklabError::Compute(error.to_string()))?;
    std::fs::write(&path, json).map_err(|source| MarklabError::io(&path, source))
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
