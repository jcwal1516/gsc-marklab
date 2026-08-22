use std::path::Path;

use crate::{
    config::OutputSection,
    errors::{MarklabError, Result},
};

use super::{
    artifact_io::{write_available_json, write_json, write_timing_sidecar},
    MultimodalResult,
};

pub(super) fn write_core_multimodal_outputs(
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
            crate::io::geojson::write_neighborhood_territories(
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
                &result.case_id,
                &result.timepoint,
                &result.protein,
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

    write_timing_sidecar(out, &result.timings)
}
