use std::{fs, path::Path};

use serde::Serialize;

use crate::{
    config::AnalysisConfig,
    errors::{MarklabError, Result},
    multimodal::{
        cell_table::{
            load_cellvit_he_cell_table_csv, load_he_cell_table_csv, load_ihc_cell_table_csv,
        },
        MultimodalEngine, MultimodalInput, NullModelSensitivityResult, RegistrationExtrapolation,
        RegistrationResidual,
    },
    output::{MultimodalResult, OutputWriter, ResultDocument},
    registration::landmarks::LandmarkPair,
};

use super::super::{HeInputFormat, LandmarkRow, MultimodalAnalyzeRequest};

pub(in crate::cli) fn run(request: MultimodalAnalyzeRequest) -> Result<()> {
    let MultimodalAnalyzeRequest {
        he_cells,
        ihc_cells,
        landmarks,
        config,
        out,
        case_id,
        timepoint,
        protein,
        he_format,
        cellvit_min_probability,
    } = request;
    let config = AnalysisConfig::from_toml_path(&config)?;
    let engine = MultimodalEngine::new(config.clone())?;
    #[cfg(not(feature = "parquet"))]
    if config.output.write_parquet_curves {
        bail!("Multimodal parquet output requires the parquet feature");
    }

    let he = match he_format {
        HeInputFormat::HeCsv => load_he_cell_table_csv(&he_cells)?,
        HeInputFormat::CellvitCsv => {
            load_cellvit_he_cell_table_csv(&he_cells, cellvit_min_probability)?
        }
    };
    let ihc = load_ihc_cell_table_csv(&ihc_cells)?;
    let landmarks = read_landmark_pairs(&landmarks)?;
    let run = engine.analyze_run(&MultimodalInput {
        he_cells: he,
        ihc_cells: ihc,
        landmarks,
        case_id,
        timepoint,
        protein,
    })?;
    let result = run.result;
    OutputWriter::write(
        &ResultDocument::multimodal(result.clone()),
        &out,
        &config.output,
    )?;
    write_registration_qc_sidecars(
        &out,
        &run.transform,
        &run.registration_residuals,
        &run.extrapolation,
    )?;
    write_pretty_json(
        &out.join("null_model_sensitivity.json"),
        &run.null_model_sensitivity,
    )?;
    write_multimodal_csv_sidecars(&out, &result, &run.null_model_sensitivity)?;
    Ok(())
}

pub(super) fn write_pretty_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

fn write_registration_qc_sidecars(
    out: &Path,
    transform: &crate::registration::transform::Transform2D,
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
    write_csv_records(&out.join("fused_cells.csv"), &result.fused_cells)?;
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

fn write_csv_records<T: Serialize>(path: &Path, records: &[T]) -> Result<()> {
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

fn read_landmark_pairs(path: &Path) -> Result<Vec<LandmarkPair>> {
    let mut reader = ::csv::Reader::from_path(path)?;
    let headers = reader.headers()?.clone();
    let expected_headers = ["source_x_um", "source_y_um", "target_x_um", "target_y_um"];
    if headers.iter().collect::<Vec<_>>() != expected_headers {
        bail!(
            "{}: expected landmark CSV headers source_x_um,source_y_um,target_x_um,target_y_um",
            path.display()
        );
    }

    let mut landmarks = Vec::new();
    for (index, row) in reader.deserialize::<LandmarkRow>().enumerate() {
        let row_number = index + 2;
        let row = row.map_err(|err| {
            MarklabError::Validation(format!(
                "{} row {}: invalid landmark row: {}",
                path.display(),
                row_number,
                err
            ))
        })?;
        if !row.source_x_um.is_finite()
            || !row.source_y_um.is_finite()
            || !row.target_x_um.is_finite()
            || !row.target_y_um.is_finite()
        {
            bail!(
                "{} row {}: landmark coordinates must be finite",
                path.display(),
                row_number
            );
        }
        landmarks.push(LandmarkPair::new(
            row.source_x_um,
            row.source_y_um,
            row.target_x_um,
            row.target_y_um,
        ));
    }
    Ok(landmarks)
}
