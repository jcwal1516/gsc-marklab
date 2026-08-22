use std::{fs, path::Path};

use serde::Serialize;

use crate::{
    config::AnalysisConfig,
    errors::{MarklabError, Result},
    multimodal::{
        load_cellvit_he_cell_table_csv, load_he_cell_table_csv, load_ihc_cell_table_csv,
        MultimodalEngine, MultimodalInput,
    },
    output::OutputWriter,
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
    OutputWriter::write_multimodal_run(run, &out, &config.output)?;
    Ok(())
}

pub(super) fn write_pretty_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(value)?)?;
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
