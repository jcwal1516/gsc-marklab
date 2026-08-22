use std::{fs, path::Path};

use serde::Serialize;

use crate::{
    common::finite::validate_serializable_finite,
    config::OutputSection,
    errors::{MarklabError, Result},
    multimodal::{
        CellSection, FusedCell, MultimodalAnalysisRun, NullModelSensitivityResult,
        RegistrationExtrapolation, RegistrationResidual,
    },
    registration::transform::Transform2D,
};

use super::{AnalysisResult, MultimodalResult, OutputManifest, OutputWriter, ResultDocument};

impl OutputWriter {
    pub fn write_multimodal_run(
        run: MultimodalAnalysisRun,
        out: impl AsRef<Path>,
        options: &OutputSection,
    ) -> Result<OutputManifest> {
        let MultimodalAnalysisRun {
            result,
            transform,
            graph: _,
            null_model_sensitivity,
            registration_residuals,
            extrapolation,
        } = run;
        let document = ResultDocument::multimodal(result);
        Self::write_transaction(&document, out, options, None, |staging| {
            let AnalysisResult::Multimodal(result) = &document.analysis else {
                unreachable!("multimodal constructor returned a non-multimodal document");
            };
            write_registration_qc_sidecars(
                staging,
                &transform,
                &registration_residuals,
                &extrapolation,
            )?;
            write_pretty_json(
                &staging.join("null_model_sensitivity.json"),
                &null_model_sensitivity,
            )?;
            write_multimodal_csv_sidecars(staging, result, &null_model_sensitivity)
        })
    }
}

fn write_registration_qc_sidecars(
    out: &Path,
    transform: &Transform2D,
    residuals: &[RegistrationResidual],
    extrapolation: &RegistrationExtrapolation,
) -> Result<()> {
    write_pretty_json(&out.join("registration_residuals.json"), residuals)?;
    write_csv_records(&out.join("registration_residuals.csv"), residuals)?;
    write_pretty_json(
        &out.join("registration_transform.json"),
        &serde_json::json!({
            "transform_type": transform.transform_type,
            "matrix": [
                [transform.m00, transform.m01, transform.m02],
                [transform.m10, transform.m11, transform.m12]
            ]
        }),
    )?;
    write_csv_records(
        &out.join("registration_extrapolation.csv"),
        &extrapolation.cell_flags,
    )?;
    write_pretty_json(&out.join("registration_extrapolation.json"), extrapolation)?;
    Ok(())
}

fn write_multimodal_csv_sidecars(
    out: &Path,
    result: &MultimodalResult,
    null_model_sensitivity: &[NullModelSensitivityResult],
) -> Result<()> {
    write_fused_cells_csv(&out.join("fused_cells.csv"), result)?;
    if let Some(territories) = result
        .neighborhood_territories
        .value()
        .filter(|value| !value.is_empty())
    {
        write_csv_records(&out.join("neighborhood_territories.csv"), territories)?;
    }
    if let Some(comparisons) = result
        .territory_comparisons
        .value()
        .filter(|value| !value.is_empty())
    {
        write_csv_records(&out.join("territory_comparisons.csv"), comparisons)?;
    }
    if let Some(enrichment) = result
        .neighborhood_enrichment
        .value()
        .filter(|value| !value.is_empty())
    {
        write_csv_records(&out.join("neighborhood_enrichment.csv"), enrichment)?;
    }
    if result
        .cross_interaction_curves
        .value()
        .is_some_and(|value| !value.is_empty())
    {
        write_cross_interaction_curves_csv(&out.join("cross_interaction_curves.csv"), result)?;
    }
    if result
        .territory_profiles
        .value()
        .is_some_and(|value| !value.is_empty())
    {
        write_territory_profiles_csv(&out.join("territory_profiles.csv"), result)?;
    }
    write_null_model_sensitivity_csv(
        &out.join("null_model_sensitivity.csv"),
        null_model_sensitivity,
    )?;
    Ok(())
}

#[derive(Serialize)]
struct FusedCellExportRow<'a> {
    source_section: CellSection,
    source_cell_id: &'a str,
    x_um_registered: f64,
    y_um_registered: f64,
    mmr_mark: Option<u8>,
    mmr_probability: Option<f64>,
    cell_type: Option<&'a str>,
    cell_type_probability: Option<f64>,
    same_section: bool,
    registration_error_um: Option<f64>,
    timepoint: &'a str,
    case_id: &'a str,
    protein: &'a str,
}

fn write_fused_cells_csv(path: &Path, result: &MultimodalResult) -> Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    for cell in &result.fused_cells {
        writer.serialize(fused_cell_export_row(cell, result))?;
    }
    writer.flush()?;
    Ok(())
}

fn fused_cell_export_row<'a>(
    cell: &'a FusedCell,
    result: &'a MultimodalResult,
) -> FusedCellExportRow<'a> {
    FusedCellExportRow {
        source_section: cell.source_section,
        source_cell_id: &cell.source_cell_id,
        x_um_registered: cell.x_um_registered,
        y_um_registered: cell.y_um_registered,
        mmr_mark: cell.mmr_mark,
        mmr_probability: cell.mmr_probability,
        cell_type: cell.cell_type.as_deref(),
        cell_type_probability: cell.cell_type_probability,
        same_section: cell.same_section,
        registration_error_um: cell.registration_error_um,
        timepoint: &result.timepoint,
        case_id: &result.case_id,
        protein: &result.protein,
    }
}

fn write_pretty_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    validate_serializable_finite(value).map_err(|error| {
        MarklabError::Compute(format!(
            "output artifact contains invalid floating-point data: {error}"
        ))
    })?;
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

fn write_csv_records<T: Serialize>(path: &Path, records: &[T]) -> Result<()> {
    validate_serializable_finite(records).map_err(|error| {
        MarklabError::Compute(format!(
            "output artifact contains invalid floating-point data: {error}"
        ))
    })?;
    let mut writer = csv::Writer::from_path(path)?;
    for record in records {
        writer.serialize(record)?;
    }
    writer.flush()?;
    Ok(())
}

fn write_cross_interaction_curves_csv(path: &Path, result: &MultimodalResult) -> Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record([
        "label_a",
        "label_b",
        "r_min_um",
        "r_max_um",
        "value",
        "lower_global_envelope",
        "upper_global_envelope",
        "count",
        "p_global",
    ])?;
    if let Some(curves) = result.cross_interaction_curves.value() {
        for curve in curves {
            for point in &curve.points {
                writer.serialize((
                    &curve.label_a,
                    &curve.label_b,
                    point.r_min_um,
                    point.r_max_um,
                    point.value,
                    point.lower_global_envelope,
                    point.upper_global_envelope,
                    point.count,
                    curve.p_global,
                ))?;
            }
        }
    }
    writer.flush()?;
    Ok(())
}

fn write_territory_profiles_csv(path: &Path, result: &MultimodalResult) -> Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record([
        "territory_id",
        "label",
        "fraction",
        "count",
        "below_registration_resolution",
    ])?;
    if let Some(profiles) = result.territory_profiles.value() {
        for profile in profiles {
            for fraction in &profile.cell_type_fractions {
                writer.serialize((
                    profile.territory_id,
                    &fraction.label,
                    fraction.fraction,
                    fraction.count,
                    profile.below_registration_resolution,
                ))?;
            }
        }
    }
    writer.flush()?;
    Ok(())
}

fn write_null_model_sensitivity_csv(
    path: &Path,
    sensitivity: &[NullModelSensitivityResult],
) -> Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record([
        "null_model",
        "label_a",
        "label_b",
        "observed_edges",
        "expected_edges",
        "enrichment_ratio",
        "enrichment_ratio_unavailable_reason",
        "z_score",
        "z_score_unavailable_reason",
        "p_value",
        "q_value",
    ])?;
    for model in sensitivity {
        for row in &model.results {
            writer.serialize((
                &model.null_model,
                &row.label_a,
                &row.label_b,
                row.observed_edges,
                row.expected_edges,
                row.enrichment_ratio,
                row.enrichment_ratio_unavailable_reason
                    .map(|reason| reason.as_str()),
                row.z_score,
                row.z_score_unavailable_reason.map(|reason| reason.as_str()),
                row.p_value,
                row.q_value,
            ))?;
        }
    }
    writer.flush()?;
    Ok(())
}
