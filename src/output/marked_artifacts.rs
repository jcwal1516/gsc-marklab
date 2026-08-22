use std::path::Path;

use crate::{
    api::MarkedAnalysisRun,
    config::OutputSection,
    errors::{MarklabError, Result},
};

#[cfg(feature = "cli")]
use super::manifest::RunManifestContext;
use super::{
    artifact_io::write_timing_sidecar, MarkedPatternResult, OutputManifest, OutputWriter,
    ResultDocument,
};

pub(super) fn write_core_marked_outputs(
    result: &MarkedPatternResult,
    out: &Path,
    options: &OutputSection,
) -> Result<()> {
    std::fs::create_dir_all(out).map_err(|source| MarklabError::io(out, source))?;

    #[cfg(not(feature = "parquet"))]
    if options.write_parquet_curves {
        return Err(MarklabError::Config(
            "Parquet curve output requires the parquet feature".into(),
        ));
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
        result.write_mark_pair_covariance_parquet(out)?;
        result.write_scale_energy_parquet(out)?;
    }
    if options.write_geojson_territories {
        if let Some(territories) = result.residual_territories.value() {
            crate::io::geojson::write_residual_territories(
                territories,
                out.join("residual_territories.geojson"),
            )?;
        }
    }
    if options.write_figures {
        super::figures::write(result, out)?;
    }

    write_timing_sidecar(out, &result.timings)
}

impl OutputWriter {
    pub fn write_marked_run(
        run: MarkedAnalysisRun,
        out: impl AsRef<Path>,
        options: &OutputSection,
    ) -> Result<OutputManifest> {
        Self::write(&ResultDocument::marked(run.result), out, options)
    }

    #[cfg(feature = "cli")]
    pub(crate) fn write_marked_run_with_manifest_context<F>(
        run: MarkedAnalysisRun,
        out: impl AsRef<Path>,
        options: &OutputSection,
        manifest_context: RunManifestContext,
        additional_artifacts: F,
    ) -> Result<OutputManifest>
    where
        F: FnOnce(&Path) -> Result<()>,
    {
        Self::write_transaction(
            &ResultDocument::marked(run.result),
            out,
            options,
            Some(manifest_context),
            additional_artifacts,
        )
    }
}
